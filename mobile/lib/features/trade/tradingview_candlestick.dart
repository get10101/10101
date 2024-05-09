import 'package:flutter/material.dart';
import 'package:flutter_inappwebview/flutter_inappwebview.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:get_10101/main.dart';

class TradingViewCandlestick extends StatefulWidget {
  const TradingViewCandlestick({
    super.key,
  });

  @override
  State<TradingViewCandlestick> createState() => _TradingViewCandlestickState();
}

class _TradingViewCandlestickState extends State<TradingViewCandlestick> {
  final GlobalKey webViewKey = GlobalKey();
  InAppWebViewController? webViewController;

  double progress = 0;

  /// place holder if loading fails
  late String html = """<html lang="en"><body><p>Tradingview chart not found</p></body></html>""";
  // this url doesn't matter, it just has to exist
  final baseUrl = WebUri("https://10101.finance/");

  @override
  void initState() {
    super.initState();

    const Color bg = appBackgroundColor;
    String rgba = "rgba(${bg.red}, ${bg.green}, ${bg.blue}, ${255.0 / bg.alpha})";
    html = '''
      <html lang="en">
        <head>
          <title></title>
          <meta name="viewport" content="width=device-width, initial-scale=1.0">
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
        </head>
        
        <body>
        
    
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
  }

  @override
  Widget build(BuildContext context) {
    return Stack(
      children: [
        InAppWebView(
          key: webViewKey,
          onWebViewCreated: (controller) {
            webViewController = controller;
            webViewController!.loadData(data: html, baseUrl: baseUrl, historyUrl: baseUrl);
          },
          shouldOverrideUrlLoading: (controller, navigationAction) async {
            var uri = navigationAction.request.url!;

            if (uri.toString().startsWith("https://www.tradingview.com/chart")) {
              // this is the link to the external chart, we want to open this in an external window
              if (await canLaunchUrl(uri)) {
                // Launch the App
                await launchUrl(uri, mode: LaunchMode.externalApplication);
                // and cancel the request
                return NavigationActionPolicy.CANCEL;
              }
            }

            return NavigationActionPolicy.ALLOW;
          },
          onProgressChanged: (controller, progress) {
            setState(() {
              this.progress = progress / 100;
            });
          },
        ),
        progress < 1.0 ? LinearProgressIndicator(value: progress) : Container(),
      ],
    );
  }
}
