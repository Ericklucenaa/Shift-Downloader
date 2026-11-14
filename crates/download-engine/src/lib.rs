//! Adaptador do aria2: processo supervisionado + cliente JSON-RPC.
//!
//! O aria2 escuta só em 127.0.0.1, numa porta aleatória, com segredo aleatório
//! por execução. Ele guarda o checkpoint de bytes (arquivo `.aria2`) e a sessão;
//! o SHIFT guarda o estado das tarefas e o GID que ele mesmo escolheu, para
//! poder recriar a tarefa no motor se a sessão se perder.

mod client;
mod process;

pub use client::{Aria2Client, Aria2File, Aria2Status, RpcError};
pub use process::{Aria2Config, Aria2Process, EngineEvent, SpawnError};

/// GID do aria2: 16 dígitos hexadecimais. Gerado pelo SHIFT a partir do UUID da tarefa
/// (ou de um aleatório), para ser estável entre reinícios.
pub fn gid_from_bytes(bytes: &[u8; 16]) -> String {
    let mut x = 0u64;
    for (i, b) in bytes.iter().enumerate() {
        x ^= (*b as u64) << ((i % 8) * 8);
    }
    // GID 0 é inválido para o aria2.
    if x == 0 {
        x = 1;
    }
    format!("{x:016x}")
}

pub fn is_valid_gid(s: &str) -> bool {
    s.len() == 16 && s.bytes().all(|b| b.is_ascii_hexdigit()) && s != "0000000000000000"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gid_tem_16_hex_e_e_estavel() {
        let g = gid_from_bytes(&[7u8; 16]);
        assert!(is_valid_gid(&g), "{g}");
        assert_eq!(g, gid_from_bytes(&[7u8; 16]));
        assert!(is_valid_gid(&gid_from_bytes(&[0u8; 16])));
        assert!(!is_valid_gid("xyz"));
    }
}
