#![allow(unused)]

mod client;
use client::*;

use {
    anchor_lang::{
        prelude::*,
        solana_program::{instruction::Instruction, system_program},
        InstructionData, ToAccountMetas,
    },
    anyhow::Result,
    bincode::serialize,
    clockwork_thread_program::state::{LookupTables, Thread, Trigger, PAYER_PUBKEY},
    serde_json::json,
    solana_address_lookup_table_program::{
        instruction::{create_lookup_table, extend_lookup_table},
        state::AddressLookupTable,
    },
    solana_client::{rpc_config::RpcSendTransactionConfig, rpc_request::RpcRequest},
    solana_sdk::{
        address_lookup_table_account::AddressLookupTableAccount,
        commitment_config::{CommitmentConfig, CommitmentLevel},
        message::{v0, VersionedMessage},
        native_token::LAMPORTS_PER_SOL,
        signature::{read_keypair_file, Keypair, Signature, Signer},
        signers::Signers,
        slot_history::Slot,
        system_instruction,
        transaction::{Transaction, VersionedTransaction},
    },
    solana_transaction_status::UiTransactionEncoding,
    std::{str::FromStr, thread, time},
};

fn main() -> Result<()> {
    // Creating a Client with your default paper keypair as payer
    let client = default_client();
    let app_localnet_simul_pk =
        Pubkey::from_str("GuJVu6wky7zeVaPkGaasC5vx1eVoiySbEv7UFKZAu837").unwrap();
    client.airdrop(&app_localnet_simul_pk, LAMPORTS_PER_SOL)?;

    // create a lookup table
    println!("Create the address lookup table");
    let recent_slot = client
        .get_slot_with_commitment(CommitmentConfig::finalized())
        .unwrap();
    let lut_auth = client.payer_pubkey();
    let (create_ix, lut) = solana_address_lookup_table_program::instruction::create_lookup_table(
        lut_auth,
        client.payer_pubkey(),
        recent_slot,
    );
    let latest_blockhash = client.get_latest_blockhash().unwrap();
    client
        .send_and_confirm_transaction(&Transaction::new_signed_with_payer(
            &[create_ix],
            Some(&client.payer_pubkey()),
            &[client.payer()],
            latest_blockhash,
        ))
        .unwrap();

    // create the instructions to give to the thread and 
    // extend the lookup table we created with accounts to be used 
    // by the instructions supplied to the thread
    let mut ixs = Vec::new();
    let mut keys: Vec<Pubkey> = Vec::new();
    keys.extend([system_program::ID, clockwork_thread_program::ID]);
    for i in 0..7 {
        let kp = Keypair::new();
        let target_ix =
            system_instruction::transfer(&PAYER_PUBKEY, &kp.pubkey(), LAMPORTS_PER_SOL / 100);
        keys.push(kp.pubkey());
        ixs.push(target_ix);
    }
    println!("Extend the address lookup table");
    let mut signature = Signature::default();
    let latest_blockhash = client.get_latest_blockhash().unwrap();
    println!("keys: {:?}", keys);
    let extend_ix = solana_address_lookup_table_program::instruction::extend_lookup_table(
        lut,
        lut_auth,
        Some(client.payer_pubkey()),
        keys.into(),
    );
    signature = client
        .send_and_confirm_transaction(&Transaction::new_signed_with_payer(
            &[extend_ix],
            Some(&client.payer_pubkey()),
            &[&client.payer],
            latest_blockhash,
        ))
        .unwrap();

    println!("Wait some arbitrary amount of time to please the address lookup table");
    thread::sleep(time::Duration::from_secs(10));

    
    // create two similar threads - one of which will use the lookup table we created   
    let ts = chrono::Local::now();
    let thread_auth = client.payer_pubkey();
    // thread with lookup table
    let thread_with_lut_id = format!("{}_{}", "with_lut", ts.format("%d_%H:%M:%S"));
    let thread_with_lut = Thread::pubkey(thread_auth, thread_with_lut_id.clone().into());    
    // thread without lut
    let thread_without_lut_id = format!("{}_{}", "wo_lut", ts.format("%d_%H:%M:%S"));
    let thread_without_lut = Thread::pubkey(thread_auth, thread_without_lut_id.clone().into());

    let thread_with_lut_create_ix = Instruction {
        program_id: clockwork_thread_program::ID,
        accounts: clockwork_thread_program::accounts::ThreadCreate {
            authority: client.payer_pubkey(),
            payer: client.payer_pubkey(),
            system_program: system_program::ID,
            thread: thread_with_lut,
        }
        .to_account_metas(Some(false)),
        data: clockwork_thread_program::instruction::ThreadCreate {
            amount: LAMPORTS_PER_SOL,
            id: thread_with_lut_id.into(),
            instructions: ixs.iter().map(|e| e.clone().into()).collect(),
            trigger: Trigger::Cron {
                schedule: "*/10 * * * * * *".into(),
                skippable: true,
            },
        }
        .data(),
    };
    println!("thread with lookup table: {:#?}", thread_with_lut);

    let thread_without_lut_create_ix = Instruction {
        program_id: clockwork_thread_program::ID,
        accounts: clockwork_thread_program::accounts::ThreadCreate {
            authority: client.payer_pubkey(),
            payer: client.payer_pubkey(),
            system_program: system_program::ID,
            thread: thread_without_lut,
        }
        .to_account_metas(Some(false)),
        data: clockwork_thread_program::instruction::ThreadCreate {
            amount: LAMPORTS_PER_SOL,
            id: thread_without_lut_id.into(),
            instructions: ixs.iter().map(|e| e.clone().into()).collect(),
            trigger: Trigger::Cron {
                schedule: "*/10 * * * * * *".into(),
                skippable: true,
            },
        }
        .data(),
    };
    println!("thread without lookup table: {:#?}", thread_without_lut);

    // Add LookupTables to Thread
    let thread_lut = LookupTables::pubkey(thread_auth, thread_with_lut);
    let thread_lut_create_ix = Instruction {
        program_id: clockwork_thread_program::ID,
        accounts: clockwork_thread_program::accounts::LookupTablesCreate {
            authority: client.payer_pubkey(),
            payer: client.payer_pubkey(),
            system_program: system_program::ID,
            thread: thread_with_lut,
            lookup_tables: thread_lut,
        }
        .to_account_metas(Some(false)),
        data: clockwork_thread_program::instruction::ThreadLookupTablesCreate {
            address_lookup_tables: vec![lut],
        }
        .data(),
    };

    let create_thread_with_lut_ixs = [thread_with_lut_create_ix, thread_lut_create_ix];
    let create_thread_without_lut_ix = [thread_without_lut_create_ix];

    let mut signers = vec![&client.payer];

    // create threads
    signature = client
        .send_and_confirm_transaction(&Transaction::new_signed_with_payer(
            &create_thread_with_lut_ixs,
            Some(&client.payer_pubkey()),
            &[&client.payer],
            latest_blockhash,
        ))
        .unwrap();
    println!("thread with lut created: https://explorer.solana.com/tx/{}?cluster=custom&customUrl=http%3A%2F%2Flocalhost%3A8899", signature);
    signature = client
        .send_and_confirm_transaction(&Transaction::new_signed_with_payer(
            &create_thread_without_lut_ix,
            Some(&client.payer_pubkey()),
            &[&client.payer],
            latest_blockhash,
        ))
        .unwrap();
    println!("thread w/o lut created: https://explorer.solana.com/tx/{}?cluster=custom&customUrl=http%3A%2F%2Flocalhost%3A8899", signature);

    print!("Waiting for threads to execute...");
    // we assume each thread should have executed at least once by 30 sec with 10 sec trigger
    thread::sleep(time::Duration::from_secs(30));

    // get latest signature for each thread
    let thread_with_lut_sig = client.get_signatures_for_address(&thread_with_lut)?[0].signature.clone();
    let thread_without_lut_sig = client.get_signatures_for_address(&thread_without_lut)?[0].signature.clone();

    // inspect each signature in the explorer
    println!("Inspect thread with lut latest sig: https://explorer.solana.com/tx/{thread_with_lut_sig}/inspect?cluster=custom&customUrl=http%3A%2F%2Flocalhost%3A8899");
    println!("Inspect thread without lut latest sig: https://explorer.solana.com/tx/{thread_without_lut_sig}/inspect?cluster=custom&customUrl=http%3A%2F%2Flocalhost%3A8899");

    Ok(())
}

fn default_client() -> Client {
    let config_file = solana_cli_config::CONFIG_FILE.as_ref().unwrap().as_str();
    let config = solana_cli_config::Config::load(config_file).unwrap();
    let keypair = read_keypair_file(&config.keypair_path).unwrap();
    Client::new(keypair, config.json_rpc_url)
}
