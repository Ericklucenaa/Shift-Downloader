//! Política de destinos de rede (referência: OWASP SSRF Prevention Cheat Sheet).
//!
//! Um gerenciador de downloads precisa aceitar endereços da rede local quando o
//! próprio usuário os digita (NAS, servidor doméstico). O risco está em um site
//! público *redirecionar* o download para dentro da rede do usuário. Regra:
//! - destino digitado: público, privado e loopback são aceitos; link-local
//!   (169.254.x.x, metadados de nuvem) e endereços especiais nunca;
//! - redirecionamento: não pode "entrar" na rede (público → privado/loopback)
//!   nem ir para link-local ou especial.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IpClass {
    Public,
    Private,
    Loopback,
    LinkLocal,
    /// 0.0.0.0, multicast, broadcast, documentação, reservados.
    Special,
}

pub fn classify_ip(ip: IpAddr) -> IpClass {
    match ip {
        IpAddr::V4(v4) => classify_v4(v4),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return classify_v4(v4);
            }
            classify_v6(v6)
        }
    }
}

fn classify_v4(ip: Ipv4Addr) -> IpClass {
    let o = ip.octets();
    if ip.is_loopback() {
        IpClass::Loopback
    } else if ip.is_link_local() {
        IpClass::LinkLocal
    } else if ip.is_unspecified() || ip.is_broadcast() || ip.is_multicast() || ip.is_documentation() || o[0] >= 240 || o[0] == 0 {
        IpClass::Special
    } else if ip.is_private() || (o[0] == 100 && (64..128).contains(&o[1])) {
        // 100.64.0.0/10: CGNAT, também é rede interna de provedores.
        IpClass::Private
    } else {
        IpClass::Public
    }
}

fn classify_v6(ip: Ipv6Addr) -> IpClass {
    let s = ip.segments();
    if ip.is_loopback() {
        IpClass::Loopback
    } else if ip.is_unspecified() || ip.is_multicast() {
        IpClass::Special
    } else if (s[0] & 0xffc0) == 0xfe80 {
        IpClass::LinkLocal
    } else if (s[0] & 0xfe00) == 0xfc00 {
        IpClass::Private // ULA fc00::/7
    } else if s[0] == 0x2001 && s[1] == 0x0db8 {
        IpClass::Special
    } else {
        IpClass::Public
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RedirectVerdict {
    Allow,
    Block(String),
}

/// Destino digitado pelo usuário (todas as IPs para as quais o host resolve).
pub fn check_initial_target(ips: &[IpAddr]) -> RedirectVerdict {
    for ip in ips {
        match classify_ip(*ip) {
            IpClass::LinkLocal => return RedirectVerdict::Block(format!("{ip} é um endereço link-local (usado por serviços internos do sistema e da nuvem)")),
            IpClass::Special => return RedirectVerdict::Block(format!("{ip} não é um destino de download válido")),
            _ => {}
        }
    }
    RedirectVerdict::Allow
}

/// Um salto de redirecionamento: `from` é a classe mais interna já visitada.
pub fn check_redirect(from: IpClass, to_ips: &[IpAddr]) -> RedirectVerdict {
    if let RedirectVerdict::Block(r) = check_initial_target(to_ips) {
        return RedirectVerdict::Block(r);
    }
    for ip in to_ips {
        let to = classify_ip(*ip);
        if from == IpClass::Public && matches!(to, IpClass::Private | IpClass::Loopback) {
            return RedirectVerdict::Block(format!(
                "um site público tentou redirecionar o download para {ip}, um endereço da sua rede interna"
            ));
        }
    }
    RedirectVerdict::Allow
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn classifica_enderecos() {
        assert_eq!(classify_ip(ip("8.8.8.8")), IpClass::Public);
        assert_eq!(classify_ip(ip("192.168.0.10")), IpClass::Private);
        assert_eq!(classify_ip(ip("10.1.2.3")), IpClass::Private);
        assert_eq!(classify_ip(ip("172.20.0.1")), IpClass::Private);
        assert_eq!(classify_ip(ip("100.100.0.1")), IpClass::Private);
        assert_eq!(classify_ip(ip("127.0.0.1")), IpClass::Loopback);
        assert_eq!(classify_ip(ip("169.254.169.254")), IpClass::LinkLocal);
        assert_eq!(classify_ip(ip("0.0.0.0")), IpClass::Special);
        assert_eq!(classify_ip(ip("::1")), IpClass::Loopback);
        assert_eq!(classify_ip(ip("fe80::1")), IpClass::LinkLocal);
        assert_eq!(classify_ip(ip("fd00::1")), IpClass::Private);
        assert_eq!(classify_ip(ip("::ffff:192.168.1.1")), IpClass::Private);
        assert_eq!(classify_ip(ip("2606:4700::1111")), IpClass::Public);
    }

    #[test]
    fn usuario_pode_baixar_da_rede_local_mas_nao_de_metadados() {
        assert_eq!(check_initial_target(&[ip("192.168.0.5")]), RedirectVerdict::Allow);
        assert_eq!(check_initial_target(&[ip("127.0.0.1")]), RedirectVerdict::Allow);
        assert!(matches!(check_initial_target(&[ip("169.254.169.254")]), RedirectVerdict::Block(_)));
    }

    #[test]
    fn redirecionamento_publico_para_interno_e_bloqueado() {
        assert!(matches!(check_redirect(IpClass::Public, &[ip("10.0.0.1")]), RedirectVerdict::Block(_)));
        assert!(matches!(check_redirect(IpClass::Public, &[ip("127.0.0.1")]), RedirectVerdict::Block(_)));
        assert!(matches!(check_redirect(IpClass::Loopback, &[ip("169.254.169.254")]), RedirectVerdict::Block(_)));
        assert_eq!(check_redirect(IpClass::Public, &[ip("1.1.1.1")]), RedirectVerdict::Allow);
        assert_eq!(check_redirect(IpClass::Private, &[ip("192.168.0.9")]), RedirectVerdict::Allow);
    }
}
