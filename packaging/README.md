# Packaging filo for distribution

`filo` is published on [crates.io](https://crates.io/crates/filo-rs) as `filo-rs`
(binary name `filo`). This directory holds everything needed to distribute it
through **Homebrew**, the **AUR** (yay/pacman), and **apt** (`.deb`).

Everything here builds on **prebuilt binaries attached to each GitHub Release**.
That part is automated by `.github/workflows/tag-release.yml` — pushing a `vX.Y.Z`
tag now produces:

| Asset pattern                                   | Used by            |
| ----------------------------------------------- | ------------------ |
| `filo-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz`   | Homebrew, AUR      |
| `filo-vX.Y.Z-aarch64-unknown-linux-gnu.tar.gz`  | Homebrew, AUR      |
| `filo-vX.Y.Z-x86_64-apple-darwin.tar.gz`        | Homebrew (Intel)   |
| `filo-vX.Y.Z-aarch64-apple-darwin.tar.gz`       | Homebrew (Apple S) |
| `*.sha256` (one per archive)                     | checksum updates   |
| `filo-rs_X.Y.Z-1_amd64.deb`                      | apt / dpkg         |

> Each archive has a matching `.sha256` file in the same release. Use those
> values to fill the `REPLACE_WITH_..._SHA256` placeholders below.

---

## 1. Homebrew (personal tap)

You don't yet qualify for `homebrew-core` (it needs notability — roughly 75+
stars, sustained usage). A **personal tap** works immediately and is the normal
starting point.

> The Homebrew formula is named **`filo-rs`** (the plain `filo` name is already
> taken on Homebrew, and it matches the crates.io name). The installed binary is
> still `filo` — users keep typing `filo`.

1. Create a new GitHub repo named **`homebrew-filo`** (the `homebrew-` prefix is
   required — the tap is then referenced as `Kanishk2207/filo`).
2. Add `Formula/filo-rs.rb` (copy from [`homebrew/filo-rs.rb`](homebrew/filo-rs.rb)).
3. Fill in the four `sha256` values from the release's `.sha256` assets, e.g.:
   ```bash
   curl -sL https://github.com/Kanishk2207/filo/releases/download/v0.1.0/filo-v0.1.0-x86_64-apple-darwin.tar.gz.sha256
   ```
4. Bump `version` in the formula on every release and refresh the four hashes
   (or automate it — see "Automating the Homebrew bump" below).

Users then install with:
```bash
brew install Kanishk2207/filo/filo-rs
```

### Automating the Homebrew bump

The `bump-homebrew` job in `.github/workflows/tag-release.yml` does steps 3–4
for you on every release: it downloads each platform binary, computes its
sha256, regenerates `Formula/filo-rs.rb` with the new version + hashes, and
pushes it to the `homebrew-filo` repo. After setup you never hand-edit hashes.

**One-time setup — create a token so the `filo` workflow can write to the tap:**

1. GitHub → your avatar → **Settings** → **Developer settings** →
   **Personal access tokens** → **Fine-grained tokens** → **Generate new token**.
2. **Resource owner:** your account (`Kanishk2207`).
3. **Repository access:** *Only select repositories* → pick **`homebrew-filo`**.
4. **Permissions:** Repository permissions → **Contents** → **Read and write**.
5. Generate and copy the token (you only see it once).
6. In the **`filo`** repo → **Settings** → **Secrets and variables** →
   **Actions** → **New repository secret**. Name it exactly
   **`HOMEBREW_TAP_TOKEN`** and paste the token.

That's it. The default `GITHUB_TOKEN` can't write to another repo, which is why
this separate token is needed.

> By default the bump runs for **every** `v*` tag (including `develop` betas), so
> Homebrew always has your latest build. To make Homebrew track **stable**
> releases only, edit the `bump-homebrew` job per the comment at its top.

---

## 2. AUR — yay / pacman (Arch)

The AUR hosts user-submitted `PKGBUILD`s. `filo-bin` installs the prebuilt
release binary (no Rust toolchain needed on the user's machine).

**One-time setup:** add your SSH public key at <https://aur.archlinux.org> →
My Account.

**Publish:**
```bash
# Clone the (empty) AUR package repo — the name is the package name.
git clone ssh://aur@aur.archlinux.org/filo-bin.git
cd filo-bin

# Copy PKGBUILD from packaging/aur/filo-bin/, fill in the sha256sums
# (or set them to 'SKIP' temporarily), then:
makepkg -si              # local build + install test
updpkgsums               # auto-fills sha256sums from the source URLs
makepkg --printsrcinfo > .SRCINFO   # REQUIRED — AUR rejects pushes without it

git add PKGBUILD .SRCINFO
git commit -m "filo-bin 0.1.0"
git push
```

Users then install with `yay -S filo-bin` (or `paru -S filo-bin`).

On each new release: bump `pkgver`, reset `pkgrel=1`, run `updpkgsums`,
regenerate `.SRCINFO`, commit, push.

> A from-source `filo` and a `filo-git` package are also conventional, but
> `filo-bin` covers the fast common case.

---

## 3. apt / `.deb`

The release workflow builds a `.deb` with [`cargo-deb`](https://github.com/kornelski/cargo-deb)
and attaches it (config lives in `[package.metadata.deb]` in `Cargo.toml`).

**Simplest for users — direct install:**
```bash
curl -LO https://github.com/Kanishk2207/filo/releases/download/v0.1.0/filo-rs_0.1.0-1_amd64.deb
sudo dpkg -i filo-rs_0.1.0-1_amd64.deb
```

**For a true `apt install filo` experience** you need a hosted APT repository.
Getting into *official* Debian/Ubuntu repos requires a sponsor and a long
review, so the practical options are:
- A free hosted repo on **Cloudsmith** or **packagecloud.io** (they give you the
  `apt-add-repository` one-liner).
- A self-hosted repo on **GitHub Pages** built with `aptly` or `reprepro` and
  signed with a GPG key.

Start with the direct `.deb` download; add a hosted repo later if there's demand.

> Only an `amd64` `.deb` is built today. An `arm64` `.deb` can be added with a
> cross-build (`cargo deb --target aarch64-unknown-linux-gnu`) once needed.

---

## Release checklist

1. Bump `version` in `Cargo.toml`; commit.
2. Publish to crates.io via the **Publish Crate** workflow (or `cargo publish`).
3. Tag and push: `git tag v0.1.0 && git push origin v0.1.0`.
   - `tag-release.yml` creates the GitHub Release and attaches all binaries +
     the `.deb`.
4. Update the Homebrew formula hashes + version → push to `homebrew-filo`.
5. Bump the AUR `filo-bin` package (`updpkgsums`, `.SRCINFO`) → push.
