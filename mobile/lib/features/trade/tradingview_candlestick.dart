import 'dart:io';

import 'package:flutter/material.dart';
import 'package:get_10101/common/global_keys.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/main.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:webview_flutter/webview_flutter.dart';

class TradingViewCandlestick extends StatefulWidget {
  const TradingViewCandlestick({super.key});

  @override
  State<StatefulWidget> createState() => _TradingViewCandlestickState();
}

class _TradingViewCandlestickState extends State<TradingViewCandlestick> {
  late final WebViewController controller;

  static bool enabled() => Platform.isAndroid || Platform.isIOS;

  @override
  void initState() {
    super.initState();

    const Color bg = appBackgroundColor;
    String rgba = "rgba(${bg.red}, ${bg.green}, ${bg.blue}, ${255.0 / bg.alpha})";
    String html = '''
      <html>
      <head>
        <meta name="viewport" content="width=device-width, initial-scale=1.0">
      </head>
      <body>
      <style>
        body {
          overflow: hidden;
        }
        
        html, body, #container, .tradingview-widget-container {
          background-color: $rgba;
        }
        
        iframe {
          height: 100% !important;
        }
        
        .tradingview-widget-copyright {
            display: none;
        }
      </style>
  
      <div id="container">
      <!-- TradingView Widget BEGIN -->
      <div class="tradingview-widget-container" style="height:100%;width:100%">
        <div class="tradingview-widget-container__widget" style="height:100%;width:100%"></div>
        <div class="tradingview-widget-copyright"><a href="https://www.tradingview.com/" rel="noopener nofollow" target="_blank"><span class="blue-text">Track all markets on TradingView</span></a></div>
        <script type="text/javascript" src="https://s3.tradingview.com/external-embedding/embed-widget-advanced-chart.js" async>
        {
        "autosize": true,
        "symbol": "BITMEX:XBT",
        "interval": "D",
        "timezone": "Etc/UTC",
        "theme": "light",
        "style": "1",
        "locale": "en",
        "enable_publishing": false,
        "backgroundColor": "$rgba",
        "hide_top_toolbar": true,
        "hide_legend": true,
        "allow_symbol_change": false,
        "save_image": false,
        "calendar": false,
        "support_host": "https://www.tradingview.com"
      }
        </script>
      </div>
      </div>
      <!-- TradingView Widget END -->
      </body>
      </html>
    ''';

    if (enabled()) {
      controller = WebViewController()
        ..setJavaScriptMode(JavaScriptMode.unrestricted)
        ..enableZoom(false)
        ..setNavigationDelegate(NavigationDelegate(
          onNavigationRequest: (req) async {
            final uri = Uri.parse(req.url);

            if (uri.scheme == "about") {
              return NavigationDecision.navigate;
            }

            if (await canLaunchUrl(uri)) {
              await launchUrl(uri, mode: LaunchMode.externalApplication);
            } else {
              showSnackBar(ScaffoldMessenger.of(rootNavigatorKey.currentContext!),
                  "Failed to open link to TradingView");
            }

            return NavigationDecision.prevent;
          },
        ))
        ..loadHtmlString(html);
    }
  }

  @override
  Widget build(BuildContext context) => enabled()
      ? WebViewWidget(controller: controller)
      : const Text("TradingView chart only supported on IOS and Android");
}
