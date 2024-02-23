import 'package:flutter/material.dart';
import 'package:flutter_inappwebview/flutter_inappwebview.dart';

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

  @override
  Widget build(BuildContext context) {
    return InAppWebView(
      key: webViewKey,
      initialFile: "assets/assets/tradingview.html",
      onWebViewCreated: (controller) {
        webViewController = controller;
      },
    );
  }
}
