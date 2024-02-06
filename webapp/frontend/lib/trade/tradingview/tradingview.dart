import 'package:flutter/material.dart';
import 'package:flutter_inappwebview/flutter_inappwebview.dart';

enum ProgressIndicatorType { circular, linear }

class TradingViewWidgetHtml extends StatefulWidget {
  const TradingViewWidgetHtml({
    required this.cryptoName,
    super.key,
  });

  final String cryptoName;
  @override
  State<TradingViewWidgetHtml> createState() => _TradingViewWidgetHtmlState();
}

class _TradingViewWidgetHtmlState extends State<TradingViewWidgetHtml> {
  final GlobalKey webViewKey = GlobalKey();
  InAppWebViewController? webViewController;
  double progress = 0;
  ProgressIndicatorType type = ProgressIndicatorType.circular;

  @override
  void initState() {
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    return Column(children: <Widget>[
      Expanded(
          child: Stack(children: [
        InAppWebView(
          key: webViewKey,
          initialFile: "assets/tradingview.html",
          onWebViewCreated: (controller) {
            webViewController = controller;
          },
        ),
      ])),
    ]);
  }
}
