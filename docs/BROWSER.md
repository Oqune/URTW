# Browser split routing / Раздельная маршрутизация браузера

Generating a profile writes `browser-routing.pac` in the selected private data
folder, with the generated profile's actual port. Routing → B edits the domains;
G regenerates. Only those suffixes use the selected core's HTTP inbound; other
hosts return DIRECT. Native profile selection does not regenerate a PAC; maintain
its matching port or generate a separate profile/PAC deliberately.
Matched domains have no automatic DIRECT fallback if the local proxy is down.
Legacy v1.0.x import PACs may include a DIRECT fallback; regenerate to update it.

## Firefox

Open Settings, search for **proxy**, open the connection settings, and choose
**Automatic proxy configuration URL**. Enter, for the default runtime:

```text
file:///C:/Users/YOUR_USER/AppData/Local/URTW/browser-routing.pac
```

Replace `YOUR_USER` or use the path of your explicitly chosen runtime. Click
Reload after replacing the PAC. This setting belongs to Firefox only.
[Mozilla's official instructions](https://support.mozilla.org/en-US/kb/connection-settings-firefox).

## Chrome / Edge

Close all instances of the browser. In a dedicated shortcut's **Properties →
Target**, append a space and this argument after the quoted executable path:

```text
--proxy-pac-url="file:///D:/URTW-data/browser-routing.pac"
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
используй логи выбранного ядра.
