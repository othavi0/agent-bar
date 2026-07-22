//! Agrupa o tier legado Waybar (spec 2026-07-21 §E): `contract.rs`
//! (formatos exportados: `modules.jsonc`/`style.css`) e `integration.rs`
//! (patch in-place do `config.jsonc`/`style.css` do usuário).
//!
//! Os módulos mantêm o IDENTIFICADOR original (`waybar_contract`,
//! `waybar_integration`) via `#[path]` — só o arquivo físico mudou de
//! lugar. Isso preserva `crate::waybar_contract::*`/`crate::waybar_integration::*`
//! em todo o resto do crate SEM editar nenhum callsite, e mantém
//! `cargo test waybar_contract`/`cargo test waybar_integration` (CLAUDE.md)
//! passando: o filtro de `cargo test` casa substring na string totalmente
//! qualificada do teste, e um `pub use ... as waybar_contract` (renomear
//! só no re-export) NÃO teria o mesmo efeito — o teste ficaria em
//! `waybar::contract::tests::…`, que não contém a substring
//! `"waybar_contract"` (o separador é `::`, não `_`).
#[path = "contract.rs"]
pub mod waybar_contract;
#[path = "integration.rs"]
pub mod waybar_integration;
