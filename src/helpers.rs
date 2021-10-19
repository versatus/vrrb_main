use crate::block;
use crate::claim::Claim;
use crate::header::BlockHeader;
use crate::network::protocol::VrrbNetworkEvent;
use crate::pool::Pool;
use crate::reward::RewardState;
use crate::state::Ledger;
use crate::state::NetworkState;
use crate::txn::Txn;
use crate::wallet::WalletAccount;
use libp2p::Multiaddr;
use ritelinked::LinkedHashMap;
use std::collections::LinkedList;
use std::fs;
use std::io;
use thiserror::Error;
use tui::{
    layout::{Alignment, Constraint},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, BorderType, Borders, Cell, List, ListItem, ListState, Paragraph, Row, Table},
};

#[derive(Error, Debug)]
pub enum JsonError {
    #[error("Error reading the json file: {0}")]
    ReadJsonError(#[from] io::Error),
    #[error("Error parsing the json file: {0}")]
    ParseJsonError(#[from] serde_json::Error),
}

pub fn render_home<'a>(addr: &Multiaddr, wallet: &WalletAccount) -> Paragraph<'a> {
    let home = Paragraph::new(vec![
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("Welcome")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("to")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::styled(
            "VRRB-CLI",
            Style::default().fg(Color::LightBlue),
        )]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw(
            "Press 'w' to access wallet, 'm' to access mining and 'n' to access network.",
        )]),
        Spans::from(vec![Span::raw("Address to dial: ")]),
        Spans::from(vec![Span::styled(
            format!("{} ", addr),
            Style::default().fg(Color::Yellow),
        )]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("Wallet Address: ")]),
        Spans::from(vec![Span::styled(
            format!("{}", wallet.addresses[&1u32].clone()),
            Style::default().fg(Color::Yellow),
        )]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("Wallet Public Key:")]),
        Spans::from(vec![Span::styled(
            format!("{}", wallet.pubkey),
            Style::default().fg(Color::Yellow),
        )]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::styled(
            "***** WRITE DOWN YOUR SECRET KEY AND PLACE IT SOMEWHERE YOU WILL NOT LOSE IT *****",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::styled(
            "***** NEVER SHARE YOUR SECRET KEY *****",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("Wallet Secret Key:")]),
        Spans::from(vec![Span::styled(
            format!("{}", wallet.get_secretkey()),
            Style::default().fg(Color::Yellow),
        )]),
    ])
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .title("Home")
            .border_type(BorderType::Plain),
    );
    home
}

pub fn render_wallet<'a>(
    wallet_list_state: &ListState,
    wallet_addresses: LinkedHashMap<u32, String>,
    credits: LinkedHashMap<String, u128>,
    debits: LinkedHashMap<String, u128>,
) -> (List<'a>, Table<'a>) {
    let addresses = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White))
        .title("Wallet Addresses")
        .border_type(BorderType::Plain);

    let items: Vec<_> = wallet_addresses
        .iter()
        .map(|(_, address)| {
            ListItem::new(Spans::from(vec![Span::styled(
                address.clone(),
                Style::default(),
            )]))
        })
        .collect();

    let selected = wallet_list_state
        .selected()
        .expect("there is always a selected address");

    let selected_address = wallet_addresses
        .get(&(selected as u32 + 1))
        .clone()
        .unwrap();

    let list = List::new(items).block(addresses).highlight_style(
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    );

    let address_credits = if let Some(credits) = credits.get(&selected_address.clone()) {
        *credits
    } else {
        0u128
    };

    let address_debits = if let Some(debits) = debits.get(&selected_address.clone()) {
        *debits
    } else {
        0u128
    };

    let balance = {
        if let Some(amount) = address_credits.checked_sub(address_debits) {
            amount
        } else {
            0u128
        }
    };

    let wallet_detail = Table::new(vec![Row::new(vec![
        Cell::from(Span::raw(format!("{}", balance))),
        Cell::from(Span::raw(format!("{}", address_credits))),
        Cell::from(Span::raw(format!("{}", address_debits))),
    ])])
    .header(Row::new(vec![
        Cell::from(Span::styled(
            "Balance",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Credits",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Debits",
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .title("Detail")
            .border_type(BorderType::Plain),
    )
    .widths(&[
        Constraint::Percentage(33),
        Constraint::Percentage(33),
        Constraint::Percentage(33),
    ]);

    (list, wallet_detail)
}

pub fn render_mining_data<'a>() -> List<'a> {
    let data = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White));
    let list = List::new(vec![ListItem::new(Spans::from(Span::raw("Mining data")))]).block(data);
    list
}

pub fn render_network_data<'a>(path: &String) -> List<'a> {
    let fields = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White))
        .title("VRRB Network Data")
        .border_type(BorderType::Plain);

    if let Ok(data) = read_from_json(path) {
        let items: Vec<ListItem> = data
            .iter()
            .map(|event| {
                ListItem::new(Spans::from(vec![Span::styled(
                    format!("{:?}", event.clone()),
                    Style::default(),
                )]))
            })
            .collect();
        let list = List::new(items).block(fields).highlight_style(
            Style::default()
                .bg(Color::Yellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        );

        return list;
    } else {
        let list = List::new(vec![ListItem::new(Spans::from(vec![Span::raw("")]))]).block(fields);
        return list;
    }
}

pub fn render_chain_list<'a>(blockchain_fields: &ListState, field_titles: Vec<String>) -> List<'a> {
    let fields = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White))
        .title("Blockchain Fields")
        .border_type(BorderType::Plain);

    let items: Vec<ListItem> = field_titles
        .iter()
        .map(|field| {
            ListItem::new(Spans::from(vec![Span::styled(
                field.clone(),
                Style::default(),
            )]))
        })
        .collect();

    let selected = blockchain_fields
        .selected()
        .expect("There is always a selected field");

    let _selected_field = field_titles.get(selected).unwrap();

    let list = List::new(items).block(fields).highlight_style(
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    );

    list
}

pub fn render_header_chain<'a>(
    last_hash_list_state: &ListState,
    chain: &LinkedList<BlockHeader>,
) -> (List<'a>, Table<'a>) {
    let header_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
        .add_modifier(Modifier::UNDERLINED);

    let headers = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White))
        .title("Last Hash")
        .border_type(BorderType::Plain);

    let items = chain
        .iter()
        .map(|header| {
            ListItem::new(Spans::from(vec![Span::styled(
                header.last_hash.clone(),
                Style::default(),
            )]))
        })
        .collect::<Vec<_>>();

    let list = List::new(items).block(headers).highlight_style(
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    );

    let table = {
        if !chain.is_empty() {
            let selected_last_hash = {
                if let Some(n) = last_hash_list_state.selected() {
                    n
                } else {
                    0
                }
            };

            let selected_block_header = {
                let mut iter = chain.iter();
                let mut i = 0;
                while i < selected_last_hash {
                    iter.next();
                    i += 1;
                }
                iter.next().unwrap()
            };

            Table::new(vec![
                Row::new(vec![
                    Cell::from(Span::raw("Last Hash")),
                    Cell::from(Span::raw(selected_block_header.last_hash.clone())),
                ]),
                Row::new(vec![
                    Cell::from(Span::raw("Block Nonce")),
                    Cell::from(Span::raw(format!(
                        "{:x}",
                        selected_block_header.block_nonce
                    ))),
                ]),
                Row::new(vec![
                    Cell::from(Span::raw("Next Block Nonce")),
                    Cell::from(Span::raw(format!(
                        "{:x}",
                        selected_block_header.next_block_nonce
                    ))),
                ]),
                Row::new(vec![
                    Cell::from(Span::raw("Block Height")),
                    Cell::from(Span::raw(selected_block_header.block_height.to_string())),
                ]),
                Row::new(vec![
                    Cell::from(Span::raw("Timestamp")),
                    Cell::from(Span::raw(selected_block_header.timestamp.to_string())),
                ]),
                Row::new(vec![
                    Cell::from(Span::raw("Txn Hash")),
                    Cell::from(Span::raw(selected_block_header.txn_hash.clone())),
                ]),
                Row::new(vec![
                    Cell::from(Span::raw("Miner")),
                    Cell::from(Span::raw(selected_block_header.claim.address.clone())),
                ]),
                Row::new(vec![
                    Cell::from(Span::raw("Claim Hash")),
                    Cell::from(Span::raw(selected_block_header.claim.hash.clone())),
                ]),
                Row::new(vec![
                    Cell::from(Span::raw("Claim Nonce")),
                    Cell::from(Span::raw(selected_block_header.claim.nonce.to_string())),
                ]),
                Row::new(vec![
                    Cell::from(Span::raw("Claim Pointer")),
                    Cell::from(Span::raw(format!(
                        "{:?}",
                        selected_block_header
                            .claim
                            .get_pointer(selected_block_header.block_nonce as u128)
                    ))),
                ]),
                Row::new(vec![
                    Cell::from(Span::raw("Claim Map Hash")),
                    Cell::from(Span::raw(format!(
                        "{:?}",
                        selected_block_header.claim_map_hash.clone()
                    ))),
                ]),
                Row::new(vec![
                    Cell::from(Span::raw("Block Reward")),
                    Cell::from(Span::raw(format!(
                        "{:?}",
                        selected_block_header.block_reward.category
                    ))),
                ]),
                Row::new(vec![
                    Cell::from(Span::raw("Next Block Reward")),
                    Cell::from(Span::raw(format!(
                        "{:?}",
                        selected_block_header.next_block_reward.category
                    ))),
                ]),
                Row::new(vec![
                    Cell::from(Span::raw("Neighbor Blocks Hash")),
                    Cell::from(Span::raw(format!(
                        "{:?}",
                        selected_block_header.neighbor_hash.clone()
                    ))),
                ]),
                Row::new(vec![
                    Cell::from(Span::raw("Block Signature")),
                    Cell::from(Span::raw(selected_block_header.signature.clone())),
                ]),
            ])
            .header(Row::new(vec![
                Cell::from(Span::styled("Field", header_style)),
                Cell::from(Span::styled("Data", header_style)),
            ]))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::White))
                    .border_type(BorderType::Plain),
            )
            .widths(&[Constraint::Percentage(15), Constraint::Percentage(85)])
        } else {
            render_empty_table()
        }
    };

    (list, table)
}

pub fn render_block_table<'a>(block: &block::Block) -> Table<'a> {
    let header_style = Style::default()
        .add_modifier(Modifier::BOLD)
        .add_modifier(Modifier::UNDERLINED);

    let first = Table::new(vec![
        Row::new(vec![
            Cell::from(Span::raw("Last Hash")),
            Cell::from(Span::raw(block.header.last_hash.clone())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Block Hash")),
            Cell::from(Span::raw(block.hash.clone())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Block Nonce")),
            Cell::from(Span::raw(format!("{:x}", block.header.block_nonce))),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Next Block Nonce")),
            Cell::from(Span::raw(format!("{:x}", block.header.next_block_nonce))),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Block Height")),
            Cell::from(Span::raw(block.header.block_height.to_string())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Timestamp")),
            Cell::from(Span::raw(block.header.timestamp.to_string())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Txn Hash")),
            Cell::from(Span::raw(block.header.txn_hash.clone())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Miner")),
            Cell::from(Span::raw(block.header.claim.address.clone())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Claim Hash")),
            Cell::from(Span::raw(block.header.claim.hash.clone())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Claim Nonce")),
            Cell::from(Span::raw(block.header.claim.nonce.to_string())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Claim Pointers")),
            Cell::from(Span::raw(format!(
                "{:?}",
                block
                    .header
                    .claim
                    .get_pointer(block.header.block_nonce as u128)
            ))),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Claim Map Hash")),
            Cell::from(Span::raw(format!("{:?}", block.header.claim_map_hash))),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Block Reward")),
            Cell::from(Span::raw(format!(
                "{:?}",
                block.header.block_reward.category
            ))),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Next Block Reward")),
            Cell::from(Span::raw(format!(
                "{:?}",
                block.header.next_block_reward.category
            ))),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Block Signature")),
            Cell::from(Span::raw(block.header.signature.clone())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Txns")),
            Cell::from(Span::raw(format!("{:?}", block.txns))),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Claims")),
            Cell::from(Span::raw(format!("{:?}", block.claims))),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Abandoned Claim")),
            Cell::from(Span::raw(format!("{:?}", block.abandoned_claim))),
        ]),
    ])
    .header(Row::new(vec![
        Cell::from(Span::styled("Field", header_style)),
        Cell::from(Span::styled("Data", header_style)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .border_type(BorderType::Plain),
    )
    .widths(&[Constraint::Percentage(20), Constraint::Percentage(80)]);
    first
}

pub fn render_invalid_blocks<'a>(
    invalid_block_list_state: &ListState,
    invalid_blocks: &LinkedHashMap<String, block::Block>,
) -> (List<'a>, Table<'a>) {
    let headers = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White))
        .title("Invalid Block Hash")
        .border_type(BorderType::Plain);

    let items = invalid_blocks
        .clone()
        .iter()
        .map(|(block_hash, _)| {
            ListItem::new(Spans::from(vec![Span::styled(
                block_hash.clone(),
                Style::default(),
            )]))
        })
        .collect::<Vec<_>>();

    let list = List::new(items).block(headers).highlight_style(
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    );

    let selected_hash_index = {
        if let Some(n) = invalid_block_list_state.selected() {
            n
        } else {
            0
        }
    };

    let selected_hash = {
        if !invalid_blocks.is_empty() {
            Some(invalid_blocks.keys().collect::<Vec<&String>>()[selected_hash_index])
        } else {
            None
        }
    };

    let selected_block = {
        if let Some(hash) = selected_hash {
            invalid_blocks.get(&hash.clone())
        } else {
            None
        }
    };

    let table = {
        if let Some(block) = selected_block {
            render_block_table(&block)
        } else {
            render_empty_table()
        }
    };

    (list, table)
}

pub fn render_command_cache<'a>(command_cache: &Vec<String>) -> List<'a> {
    let commands = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White))
        .title("Command Cache")
        .border_type(BorderType::Plain);

    let items = command_cache
        .iter()
        .map(|v| ListItem::new(Spans::from(vec![Span::styled(v.clone(), Style::default())])))
        .collect::<Vec<_>>();

    List::new(items).block(commands)
}

pub fn render_empty_table<'a>() -> Table<'a> {
    let header_style = Style::default()
        .add_modifier(Modifier::BOLD)
        .add_modifier(Modifier::UNDERLINED);

    Table::new(vec![])
        .header(Row::new(vec![
            Cell::from(Span::styled("Field", header_style)),
            Cell::from(Span::styled("Data", header_style)),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::White))
                .border_type(BorderType::Plain),
        )
        .widths(&[Constraint::Percentage(20), Constraint::Percentage(80)])
}

pub fn render_miner_list<'a>(miner_fields: &ListState, field_titles: Vec<String>) -> List<'a> {
    let fields = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White))
        .title("Miner Fields")
        .border_type(BorderType::Plain);

    let items: Vec<ListItem> = field_titles
        .iter()
        .map(|field| {
            ListItem::new(Spans::from(vec![Span::styled(
                field.clone(),
                Style::default(),
            )]))
        })
        .collect();

    let selected = miner_fields
        .selected()
        .expect("There is always a selected field");

    let _selected_field = field_titles.get(selected).unwrap();

    let list = List::new(items).block(fields).highlight_style(
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    );

    list
}

pub fn render_claim_data<'a>(claim: &Claim) -> Table<'a> {
    let header_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
        .add_modifier(Modifier::UNDERLINED);

    let table = Table::new(vec![
        Row::new(vec![
            Cell::from(Span::raw("Claim Pubkey")),
            Cell::from(Span::raw(claim.pubkey.clone())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Claim Address")),
            Cell::from(Span::raw(claim.address.clone())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Claim Hash")),
            Cell::from(Span::raw(claim.hash.clone())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Claim Nonce")),
            Cell::from(Span::raw(claim.nonce.to_string())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Claim Eligible")),
            Cell::from(Span::raw(claim.eligible.to_string())),
        ]),
    ])
    .header(Row::new(vec![
        Cell::from(Span::styled("Field", header_style)),
        Cell::from(Span::styled("Data", header_style)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .border_type(BorderType::Plain),
    )
    .widths(&[Constraint::Percentage(20), Constraint::Percentage(80)]);
    table
}

pub fn render_txn_data<'a>(txn: &Txn) -> Table<'a> {
    let header_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
        .add_modifier(Modifier::UNDERLINED);

    let mut confirmations = txn.validators.clone();
    let mut rejections = txn.validators.clone();

    confirmations.retain(|_, v| v.clone());
    rejections.retain(|_, v| !v.clone());

    let table = Table::new(vec![
        Row::new(vec![
            Cell::from(Span::raw("Txn Id")),
            Cell::from(Span::raw(txn.txn_id.clone())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Timestamp")),
            Cell::from(Span::raw(txn.txn_timestamp.to_string())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Sender Address")),
            Cell::from(Span::raw(txn.sender_address.clone())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Sender Pubkey")),
            Cell::from(Span::raw(txn.sender_public_key.clone())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Receiver Address")),
            Cell::from(Span::raw(txn.receiver_address.clone())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Token")),
            Cell::from(Span::raw(format!("{:?}", txn.txn_token.clone()))),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Amount")),
            Cell::from(Span::raw(txn.txn_amount.to_string())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Payload")),
            Cell::from(Span::raw(txn.txn_payload.clone())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Signature")),
            Cell::from(Span::raw(txn.txn_signature.clone())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Confirmations")),
            Cell::from(Span::raw(confirmations.len().to_string())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Rejections")),
            Cell::from(Span::raw(rejections.len().to_string())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Sender Nonce")),
            Cell::from(Span::raw(txn.nonce.to_string())),
        ]),
    ])
    .header(Row::new(vec![
        Cell::from(Span::styled("Field", header_style)),
        Cell::from(Span::styled("Data", header_style)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .border_type(BorderType::Plain),
    )
    .widths(&[Constraint::Percentage(20), Constraint::Percentage(80)]);
    table
}

pub fn render_claim_map<'a>(
    claim_map_list_state: &ListState,
    claim_map: &LinkedHashMap<String, Claim>,
) -> (List<'a>, Table<'a>) {
    let headers = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White))
        .title("Claim Pubkey")
        .border_type(BorderType::Plain);

    let items = claim_map
        .clone()
        .iter()
        .map(|(pubkey, _)| {
            ListItem::new(Spans::from(vec![Span::styled(
                pubkey.clone(),
                Style::default(),
            )]))
        })
        .collect::<Vec<_>>();

    let list = List::new(items).block(headers).highlight_style(
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    );

    let selected_pubkey_index = {
        if let Some(n) = claim_map_list_state.selected() {
            n
        } else {
            0
        }
    };

    let selected_pubkey = {
        if !claim_map.is_empty() {
            Some(claim_map.keys().collect::<Vec<&String>>()[selected_pubkey_index])
        } else {
            None
        }
    };

    let selected_claim = {
        if let Some(pubkey) = selected_pubkey {
            claim_map.get(&pubkey.clone())
        } else {
            None
        }
    };

    let table = {
        if let Some(claim) = selected_claim {
            render_claim_data(&claim)
        } else {
            render_empty_table()
        }
    };

    (list, table)
}

pub fn render_chain_db<'a>(path: &String) -> Paragraph<'a> {
    let chain_db = Paragraph::new(vec![
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("Welcome")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("to")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::styled(
            "VRRB-CLI",
            Style::default().fg(Color::LightBlue),
        )]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw(
            "Press 'w' to access wallet, 'm' to access mining and 'n' to access network.",
        )]),
        Spans::from(vec![Span::raw("Path to Block Archive Database: ")]),
        Spans::from(vec![Span::styled(
            format!("{} ", path),
            Style::default().fg(Color::Yellow),
        )]),
    ])
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .title("Database Path for Restoration")
            .border_type(BorderType::Plain),
    );

    chain_db
}

pub fn render_txn_pool<'a>(
    txn_pool_status_list_state: &ListState,
    txn_pool_list_state: &ListState,
    txn_pool: &Pool<String, Txn>,
) -> (List<'a>, List<'a>, Table<'a>) {
    let status_block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White))
        .title("Txn Status")
        .border_type(BorderType::Plain);

    let txn_block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White))
        .title("Txn IDs")
        .border_type(BorderType::Plain);

    let status_items = vec![
        ListItem::new(Spans::from(vec![Span::raw("Pending")])),
        ListItem::new(Spans::from(vec![Span::raw("Confirmed")])),
    ];

    let status_list = List::new(status_items).block(status_block).highlight_style(
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    );

    let (txn_list, table) = {
        if let Some(selected) = txn_pool_status_list_state.selected() {
            if selected == 0 {
                let txn_items = txn_pool
                    .pending
                    .iter()
                    .map(|(k, _)| {
                        ListItem::new(Spans::from(vec![Span::styled(k.clone(), Style::default())]))
                    })
                    .collect::<Vec<_>>();

                let txn_ids = txn_pool
                    .pending
                    .iter()
                    .map(|(k, _)| return k.clone())
                    .collect::<Vec<_>>();

                let table = {
                    if let Some(selected) = txn_pool_list_state.selected() {
                        let selected_txn_id = {
                            if txn_ids.len() > 0 {
                                Some(txn_ids[selected].clone())
                            } else {
                                None
                            }
                        };

                        if let Some(txn_id) = selected_txn_id {
                            if let Some(txn) = txn_pool.pending.get(&txn_id) {
                                render_txn_data(txn)
                            } else {
                                render_empty_table()
                            }
                        } else {
                            render_empty_table()
                        }
                    } else {
                        render_empty_table()
                    }
                };

                (List::new(txn_items).block(txn_block), table)
            } else {
                let txn_items = txn_pool
                    .confirmed
                    .iter()
                    .map(|(k, _)| {
                        ListItem::new(Spans::from(vec![Span::styled(k.clone(), Style::default())]))
                    })
                    .collect::<Vec<_>>();

                let txn_ids = txn_pool
                    .confirmed
                    .iter()
                    .map(|(k, _)| return k.clone())
                    .collect::<Vec<_>>();

                let table = {
                    if let Some(selected) = txn_pool_list_state.selected() {
                        let selected_txn_id = {
                            if txn_ids.len() > 0 {
                                Some(txn_ids[selected].clone())
                            } else {
                                None
                            }
                        };

                        if let Some(txn_id) = selected_txn_id {
                            if let Some(txn) = txn_pool.confirmed.get(&txn_id) {
                                render_txn_data(txn)
                            } else {
                                render_empty_table()
                            }
                        } else {
                            render_empty_table()
                        }
                    } else {
                        render_empty_table()
                    }
                };

                (List::new(txn_items).block(txn_block), table)
            }
        } else {
            let empty_list = render_empty_list();
            (empty_list.block(txn_block), render_empty_table())
        }
    };

    (status_list, txn_list, table)
}

pub fn render_claim_pool<'a>(
    claim_pool_status_list_state: &ListState,
    claim_pool_list_state: &ListState,
    claim_pool: &Pool<String, Claim>,
) -> (List<'a>, List<'a>, Table<'a>) {
    let status_block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White))
        .title("Claim Status")
        .border_type(BorderType::Plain);

    let claim_block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White))
        .title("Claim Pubkeys")
        .border_type(BorderType::Plain);

    let status_items = vec![
        ListItem::new(Spans::from(vec![Span::raw("Pending")])),
        ListItem::new(Spans::from(vec![Span::raw("Confirmed")])),
    ];

    let status_list = List::new(status_items).block(status_block).highlight_style(
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    );

    let (claim_list, table) = {
        if let Some(selected) = claim_pool_status_list_state.selected() {
            if selected == 0 {
                let claim_items = claim_pool
                    .pending
                    .iter()
                    .map(|(k, _)| {
                        ListItem::new(Spans::from(vec![Span::styled(k.clone(), Style::default())]))
                    })
                    .collect::<Vec<_>>();

                let pubkeys = claim_pool
                    .pending
                    .iter()
                    .map(|(k, _)| return k.clone())
                    .collect::<Vec<_>>();

                let table = {
                    if let Some(selected) = claim_pool_list_state.selected() {
                        let selected_claim_pubkey = {
                            if pubkeys.len() > 0 {
                                Some(pubkeys[selected].clone())
                            } else {
                                None
                            }
                        };

                        if let Some(pubkey) = selected_claim_pubkey {
                            if let Some(claim) = claim_pool.pending.get(&pubkey) {
                                render_claim_data(claim)
                            } else {
                                render_empty_table()
                            }
                        } else {
                            render_empty_table()
                        }
                    } else {
                        render_empty_table()
                    }
                };

                (List::new(claim_items).block(claim_block), table)
            } else {
                let claim_items = claim_pool
                    .confirmed
                    .iter()
                    .map(|(k, _)| {
                        ListItem::new(Spans::from(vec![Span::styled(k.clone(), Style::default())]))
                    })
                    .collect::<Vec<_>>();

                let pubkeys = claim_pool
                    .pending
                    .iter()
                    .map(|(k, _)| return k.clone())
                    .collect::<Vec<_>>();

                let table = {
                    if let Some(selected) = claim_pool_list_state.selected() {
                        let selected_claim_pubkey = {
                            if pubkeys.len() > 0 {
                                Some(pubkeys[selected].clone())
                            } else {
                                None
                            }
                        };

                        if let Some(pubkey) = selected_claim_pubkey {
                            if let Some(claim) = claim_pool.confirmed.get(&pubkey) {
                                render_claim_data(claim)
                            } else {
                                render_empty_table()
                            }
                        } else {
                            render_empty_table()
                        }
                    } else {
                        render_empty_table()
                    }
                };

                (List::new(claim_items).block(claim_block), table)
            }
        } else {
            let empty_list = render_empty_list();
            (empty_list.block(claim_block), render_empty_table())
        }
    };

    (status_list, claim_list, table)
}

pub fn render_reward_state<'a>(reward_state: &RewardState) -> Table<'a> {
    let header_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
        .add_modifier(Modifier::UNDERLINED);

    Table::new(vec![
        Row::new(vec![
            Cell::from(Span::raw("Epoch")),
            Cell::from(Span::raw(reward_state.epoch.to_string())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Next Epoch Block")),
            Cell::from(Span::raw(reward_state.next_epoch_block.to_string())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Current Block")),
            Cell::from(Span::raw(reward_state.current_block.to_string())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Total Nuggets Remaining")),
            Cell::from(Span::raw(reward_state.n_nuggets_remaining.to_string())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Total Veins Remaining")),
            Cell::from(Span::raw(reward_state.n_veins_remaining.to_string())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Total Motherlodes Remaining")),
            Cell::from(Span::raw(reward_state.n_motherlodes_remaining.to_string())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Current Epoch Flakes Remaining")),
            Cell::from(Span::raw(reward_state.n_flakes_current_epoch.to_string())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Current Epoch Grains Remaining")),
            Cell::from(Span::raw(reward_state.n_grains_current_epoch.to_string())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Current Epoch Nuggets Remaining")),
            Cell::from(Span::raw(reward_state.n_nuggets_current_epoch.to_string())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Current Epoch Veins Remaining")),
            Cell::from(Span::raw(reward_state.n_veins_current_epoch.to_string())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Current Epoch Motherlodes Remaining")),
            Cell::from(Span::raw(
                reward_state.n_motherlodes_current_epoch.to_string(),
            )),
        ]),
    ])
    .header(Row::new(vec![
        Cell::from(Span::styled("Field", header_style)),
        Cell::from(Span::styled("Data", header_style)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .border_type(BorderType::Plain),
    )
    .widths(&[Constraint::Percentage(35), Constraint::Percentage(65)])
}

pub fn render_network_state<'a>(network_state: &NetworkState) -> Table<'a> {
    let header_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
        .add_modifier(Modifier::UNDERLINED);

    Table::new(vec![
        Row::new(vec![
            Cell::from(Span::raw("Ledger DB Path")),
            Cell::from(Span::raw(network_state.path.clone())),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Credit Hash")),
            Cell::from(Span::raw(format!("{:?}", &network_state.credits))),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Debit Hash")),
            Cell::from(Span::raw(format!("{:?}", &network_state.debits))),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Reward State")),
            Cell::from(Span::raw("See reward_state Tab in Mining options")),
        ]),
        Row::new(vec![
            Cell::from(Span::raw("Network State Hash")),
            Cell::from(Span::raw(format!("{:?}", &network_state.state_hash))),
        ]),
    ])
    .header(Row::new(vec![
        Cell::from(Span::styled("Field", header_style)),
        Cell::from(Span::styled("Data", header_style)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .border_type(BorderType::Plain),
    )
    .widths(&[Constraint::Percentage(35), Constraint::Percentage(65)])
}

pub fn render_empty_list<'a>() -> List<'a> {
    List::new(vec![ListItem::new(Spans::from(vec![Span::raw("")]))])
}

pub fn get_credits(ledger: &Ledger) -> LinkedHashMap<String, u128> {
    ledger.credits.clone()
}

pub fn get_debits(ledger: &Ledger) -> LinkedHashMap<String, u128> {
    ledger.debits.clone()
}

pub fn read_from_json(path: &String) -> Result<Vec<VrrbNetworkEvent>, JsonError> {
    let content = fs::read_to_string(path)?;
    let parsed_json: Vec<VrrbNetworkEvent> = serde_json::from_str(&content)?;
    Ok(parsed_json)
}
