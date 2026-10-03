# Browser split routing / Раздельная маршрутизация браузера

WireGuard import writes `%LOCALAPPDATA%\URT\browser-routing.pac` (or your chosen
runtime). Only domains from `browser-vpn-domains.txt` use the Mihomo HTTP inbound;
other hosts return DIRECT. The proxy port of generated profiles is 17890. A raw
YAML using another port needs a corresponding user-maintained PAC.

## Firefox

Open Settings, search for **proxy**, open the connection settings, and choose
**Automatic proxy configuration URL**. Enter, for the default runtime:

```text
file:///C:/Users/YOUR_USER/AppData/Local/URT/browser-routing.pac
```

Replace `YOUR_USER` or use the path of your explicitly chosen runtime. Click
Reload after replacing the PAC. This setting belongs to Firefox only.
[Mozilla's official instructions](https://support.mozilla.org/en-US/kb/connection-settings-firefox).

## Chrome / Edge

Close all instances of the browser. In a dedicated shortcut's **Properties →
Target**, append a space and this argument after the quoted executable path:

```text
--proxy-pac-url="file:///D:/URT-data/browser-routing.pac"
```

Use your actual private runtime path. Launch from that shortcut, so an already
running instance does not retain previous settings. The option is documented
by [Chromium](https://www.chromium.org/developers/design-documents/network-settings/)
and [Microsoft Edge](https://learn.microsoft.com/en-us/deployedge/edge-learnmore-cmdline-options-proxy-settings).
Do not put the browser-only PAC into Windows proxy settings if you intend it
to apply only to that browser.

## Проверка

Открой сайт из своего VPN-списка и сайт вне него. PAC выбирает локальный прокси
для первого и системный путь для второго. При активном системном TUN запрос
DIRECT всё ещё может обрабатываться его правилами. HTTP 200 подтверждает ответ
сайта, но сам по себе не доказывает конкретный маршрут; для проверки маршрута
используй логи своего Mihomo.
