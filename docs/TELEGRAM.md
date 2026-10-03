# TG WS Proxy integration

URT installs the pinned [Flowseal/tg-ws-proxy](https://github.com/Flowseal/tg-ws-proxy)
Windows binary with digest verification, preserving the private portable data
directory on binary replacement. Start creates a random secret if configuration
does not exist, requires loopback binding and an unoccupied local port, and
records only the process it started. The upstream application itself enforces
one tray instance per Windows user; URT refuses to replace an existing instance.

1. Open Telegram tab, install with I, start with S.
2. L copies the private connection link; G opens Telegram's connection prompt.
3. Confirm the proxy in Telegram Desktop. This does not send any messages.
4. X stops the managed component. External instances use their own tray menu.

E changes the private port while the managed process is stopped. O opens
`tools/telegram/TgWsProxy_data/config.json` in Notepad. Stop the component before
manual edits, or use its native tray settings and restart action. Keep
`host: 127.0.0.1` and `no_secure: false`. URT never displays the secret on the
dashboard or prints a connection link to its operation log.

The official tray supports advanced Telegram DC, WebSocket, Cloudflare-domain
and Cloudflare Worker settings. These are configured in the upstream application;
URT does not create accounts, domains, Workers or external infrastructure.
Use [the upstream configuration guide](https://github.com/Flowseal/tg-ws-proxy/blob/main/docs/RU/TrayConfig.md)
for those options. Ordinary connection links require a 32-hex secret; advanced
secret formats should use the upstream tray's link.

## По-русски

Прокси работает локально для Telegram Desktop. Ссылка с адресом `127.0.0.1`
подходит этому же ПК; её нельзя использовать на телефоне как адрес удалённого
сервера. Secret и лог компонента остаются приватными. Если уже запущен другой
TG WS Proxy, управляй им через его трей; URT его не завершает. Настройки DC и
Cloudflare доступны в штатном меню оригинального приложения.
