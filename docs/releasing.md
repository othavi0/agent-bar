# Releasing agent-bar

Runbook para cortar um release. A versão é única, vinda de `Cargo.toml`
(`CARGO_PKG_VERSION`). O CI (`.github/workflows/publish.yml`) builda o binário
musl estático via `cargo-zigbuild` e anexa o tarball + `.sha256` ao GitHub Release.

## 1. Preparar a versão

1. Bumpar `version` em `Cargo.toml` (ex: `6.0.0` → `6.1.0`).
2. Bumpar `pkgver` em `packaging/aur/PKGBUILD` e em `packaging/aur/.SRCINFO` para o
   mesmo valor; resetar `pkgrel=1` em ambos.
3. Atualizar `CHANGELOG.md` (mover de `[Unreleased]` para uma seção
   `## [<version>] - YYYY-MM-DD`, formato Keep a Changelog).
4. Conferir que tudo bate:
   ```bash
   ./scripts/check-version v<version>   # Cargo.toml == PKGBUILD == tag
   cargo test && cargo clippy --all-targets -- -D warnings
   ```

## 2. Commitar + taggear

```bash
git add Cargo.toml CHANGELOG.md packaging/aur/
git commit -m "chore: release v<version>"
git push

git tag -a v<version> -m "agent-bar <version>"
git push origin v<version>
```

## 3. Publicar o GitHub Release (dispara o CI)

```bash
gh release create v<version> --title "v<version>" --notes-file <notas.md>
```

Criar o Release dispara `publish.yml` (`release: published`), que:
- instala Rust + target `x86_64-unknown-linux-musl` + zig (`mlugg/setup-zig`) +
  `cargo-zigbuild`;
- builda `cargo zigbuild --release --target x86_64-unknown-linux-musl`;
- empacota `agent-bar-<version>-x86_64.tar.gz` (binário + `scripts/agent-bar-open-terminal`
  + `icons/` + `LICENSE`, todos na raiz do arquivo) + `.sha256`;
- anexa ambos ao Release.

Acompanhar e confirmar:
```bash
gh run watch "$(gh run list --workflow=publish.yml -L1 --json databaseId -q '.[0].databaseId')" --exit-status
gh release view v<version> --json assets   # tarball + .sha256 presentes
```

## 4. Preencher o sha256 do PKGBUILD

O `sha256sums` do PKGBUILD/`.SRCINFO` fica como placeholder (64 zeros) até o
Release existir. Depois do CI:

```bash
gh release download v<version> -p '*.sha256' -D /tmp/ab-rel
HASH="$(cut -d' ' -f1 /tmp/ab-rel/*.sha256)"
# Substituir o sha256sums em packaging/aur/PKGBUILD E packaging/aur/.SRCINFO por "$HASH"
git add packaging/aur/ && git commit -m "chore: sha256 do PKGBUILD pro v<version>" && git push
```

## 5. Publicar no AUR (`agent-bar-bin`)

O AUR é um repo git **separado** (`ssh://aur@aur.archlinux.org/agent-bar-bin.git`).
Requer a conta do AUR do mantenedor + chave SSH registrada nela (a chave precisa
estar em https://aur.archlinux.org/account → SSH Public Key). Não é a chave padrão
do GitHub — registre a chave certa no AUR antes.

```bash
git clone ssh://aur@aur.archlinux.org/agent-bar-bin.git /tmp/aur-agent-bar
cd /tmp/aur-agent-bar
cp ~/Projects/agent-bar/packaging/aur/PKGBUILD .
cp ~/Projects/agent-bar/packaging/aur/.SRCINFO .
cp ~/Projects/agent-bar/packaging/aur/agent-bar-bin.install .
git add -A
git commit -m "agent-bar-bin <version>"
git push
```

Notas:
- O `.SRCINFO` precisa estar consistente com o `PKGBUILD` (mesmo `pkgver`/`sha256sums`).
  Se tiver `makepkg`, regenerar com `makepkg --printsrcinfo > .SRCINFO`; senão editar à mão.
- Se o repo AUR ainda não existir, o primeiro `git push` o cria (o `pkgbase` no
  `.SRCINFO` define o nome).
- Validar localmente (opcional, em Arch): `makepkg -si` numa cópia do tarball.

## Distribuição (sem AUR)

`install.sh` (curl|bash) e `cargo binstall agent-bar` puxam o binário direto do
GitHub Release — funcionam assim que o passo 3 termina, independente do AUR.
