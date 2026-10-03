# Verify a release / Проверка релиза

Download your ZIP plus **SHA256SUMS** and **SHA256SUMS.asc** from the same release.
The checksum manifest is signed by the release key:

```text
4DE5 73BC 3151 7B8C DC3F 7788 52EA 513D 2E48 8D74
```

Get the public key from [the maintainer's GitHub GPG profile](https://github.com/Oqune.gpg).
Import it into GPG and verify its full fingerprint against this document and a
trusted previously obtained value before trusting a release. Do not trust only
a short key ID or a key delivered beside an untrusted archive.

```powershell
gpg --import maintainer-public-key.asc
gpg --fingerprint 4DE573BC31517B8CDC3F778852EA513D2E488D74
gpg --verify SHA256SUMS.asc SHA256SUMS
Get-FileHash .\URTW-1.1.0-windows-amd64.zip -Algorithm SHA256
```

Compare the full ZIP hash with its line in SHA256SUMS. With GNU tools, use
`sha256sum --check SHA256SUMS` after downloading all listed archives.
GPG may report an untrusted identity even when the cryptographic signature is
valid; key ownership must be established separately. GPG release signatures do
not imply Windows Authenticode signing of the executable.

По-русски: сначала проверь полный fingerprint ключа, затем подпись файла
контрольных сумм, затем SHA-256 своего ZIP. При несовпадении не запускай архив.
Секретный GPG-ключ никогда не входит в исходники, CI или релиз.
