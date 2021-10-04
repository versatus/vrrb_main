// ___________________________________________________________________________
// Store State DB Chunks logic:
// ___________________________________________________________________________
Command::StoreStateDbChunk(object, data, chunk_number, total_chunks) => {
    if let Some(stashed_chunk) =
        blockchain.clone().state_update_cache.get(&object.0)
    {
        let mut stashed_chunks = stashed_chunk.clone();
        stashed_chunks.insert(chunk_number as u128, data);
        blockchain
            .state_update_cache
            .insert(object.0, stashed_chunks);
    } else {
        let mut stashed_chunks = LinkedHashMap::new();
        stashed_chunks.insert(chunk_number as u128, data);
        blockchain
            .state_update_cache
            .insert(object.0, stashed_chunks);
    }

    println!("Received block chunk: {}", object.0);

    if chunk_number == total_chunks {
        if let Some(stashed_chunk) =
            blockchain.clone().state_update_cache.get(&object.0)
        {
            let mut block_bytes_vec = vec![];
            for (_, chunk) in stashed_chunk.iter() {
                block_bytes_vec.extend(chunk);
            }
            let block = Block::from_bytes(&block_bytes_vec);
            if let Err(e) = blockchain.process_block(
                &blockchain_network_state,
                &blockchain_reward_state,
                &block,
            ) {
                println!("Error trying to process block: {:?}", e);
            } else {
                let current_blockchain = blockchain.clone();
                blockchain_network_state.dump(&block);
                let first_future_block =
                    current_blockchain.future_blocks.front();
                if let Some((_, future_block)) = first_future_block {
                    if blockchain.chain.len()
                        == future_block.header.block_height as usize
                    {
                        if let Err(e) =
                            blockchain_sender.send(Command::ProcessBacklog)
                        {
                            println!("Error sending ProcessBacklog command to blockchain thread: {:?}", e);
                        }
                    }
                }
            }
        }
    }
}

//________________________________________________________________________________________________________________