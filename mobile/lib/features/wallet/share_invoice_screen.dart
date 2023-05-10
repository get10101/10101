import 'dart:convert';
import 'dart:developer';

import 'package:f_logs/f_logs.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/features/wallet/create_invoice_screen.dart';
import 'package:get_10101/features/wallet/domain/wallet_info.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:http/http.dart' as http;
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';
import 'package:qr_flutter/qr_flutter.dart';
import 'package:share_plus/share_plus.dart';

import 'application/wallet_service.dart';

class ShareInvoiceScreen extends StatelessWidget {
  static const route = "${WalletScreen.route}/${CreateInvoiceScreen.subRouteName}/$subRouteName";
  static const subRouteName = "share_invoice";
  final WalletService walletService;
  final String invoice;

  const ShareInvoiceScreen(
      {super.key, this.walletService = const WalletService(), required this.invoice});

  @override
  Widget build(BuildContext context) {
    WalletInfo info = context.watch<WalletChangeNotifier>().walletInfo;
    final bridge.Config config = context.read<bridge.Config>();

    log("Refresh receive screen: ${info.balances.onChain}");

    const EdgeInsets buttonSpacing = EdgeInsets.symmetric(vertical: 8.0, horizontal: 24.0);

    return Scaffold(
      appBar: AppBar(title: const Text("Receive funds")),
      body: SafeArea(
          child: Container(
        constraints: const BoxConstraints.expand(),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Expanded(
              child: Column(crossAxisAlignment: CrossAxisAlignment.center, children: [
                const Padding(
                  padding: EdgeInsets.only(top: 25.0, bottom: 50.0),
                  child: Text(
                    "Share payment request",
                    style: TextStyle(fontSize: 18.0, fontWeight: FontWeight.bold),
                  ),
                ),
                Expanded(
                    child: Column(children: [
                  Center(
                    child: QrImage(
                      data: invoice,
                      version: QrVersions.auto,
                      size: 250.0,
                    ),
                  ),

                  // Faucet button, only available if we are on regtest
                  Visibility(
                    visible: config.network == "regtest",
                    child: Padding(
                        padding: const EdgeInsets.symmetric(horizontal: 35.0, vertical: 8.0),
                        child: Column(
                          children: [
                            ElevatedButton(
                              onPressed: () {
                                payInvoiceWithFaucet(invoice);
                                // Pop both create invoice screen and share invoice screen to
                                // get back to main screen
                                GoRouter.of(context).pop();
                                GoRouter.of(context).pop();
                              },
                              style: ElevatedButton.styleFrom(
                                shape: const RoundedRectangleBorder(
                                    borderRadius: BorderRadius.all(Radius.circular(5.0))),
                              ),
                              child: const Text("Pay with 10101 faucet"),
                            ),
                            const Text(
                              "It will take a few seconds until the payment arrives in your wallet",
                              textAlign: TextAlign.center,
                            )
                          ],
                        )),
                  ),
                ])),
              ]),
            ),
            Row(children: [
              Flexible(
                flex: 1,
                child: Padding(
                  padding: buttonSpacing,
                  child: OutlinedButton(
                    onPressed: () {
                      Clipboard.setData(ClipboardData(text: invoice)).then((_) {
                        ScaffoldMessenger.of(context).showSnackBar(
                            const SnackBar(content: Text('Invoice copied to clipboard')));
                      });
                    },
                    style: ElevatedButton.styleFrom(
                      shape: const RoundedRectangleBorder(
                          borderRadius: BorderRadius.all(Radius.circular(5.0))),
                    ),
                    child: Row(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: const [
                        Padding(
                          padding: EdgeInsets.symmetric(horizontal: 8.0),
                          child: Icon(Icons.copy),
                        ),
                        Text("Copy"),
                      ],
                    ),
                  ),
                ),
              ),
              Flexible(
                flex: 1,
                child: Padding(
                  padding: buttonSpacing,
                  child: OutlinedButton(
                    onPressed: () => Share.share(invoice),
                    style: ElevatedButton.styleFrom(
                      shape: const RoundedRectangleBorder(
                          borderRadius: BorderRadius.all(Radius.circular(5.0))),
                    ),
                    child: Row(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: const [
                        Padding(
                          padding: EdgeInsets.symmetric(horizontal: 8.0),
                          child: Icon(Icons.send),
                        ),
                        Text("Share"),
                      ],
                    ),
                  ),
                ),
              )
            ]),
            const Padding(
              padding: EdgeInsets.all(8.0),
              child: Center(
                child: Text(
                  "You will see the incoming payment once you send the funds.",
                  style: TextStyle(color: Colors.grey),
                ),
              ),
            ),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16.0, vertical: 8.0),
              child: ElevatedButton(
                onPressed: () {
                  // Pop both create invoice screen and share invoice screen
                  GoRouter.of(context).pop();
                  GoRouter.of(context).pop();
                },
                style: ElevatedButton.styleFrom(
                  shape: const RoundedRectangleBorder(
                      borderRadius: BorderRadius.all(Radius.circular(5.0))),
                ),
                child: const Text("Done"),
              ),
            ),
            const SizedBox(height: 30.0),
          ],
        ),
      )),
    );
  }

// Pay the generated invoice with 10101 faucet
// XXX: This is in not in Rust as this is not production-related
  Future<void> payInvoiceWithFaucet(String invoice) async {
    // Default to the faucet on the 10101 server, but allow to override it
    // locally if needed for dev testing
    // It's not populated in Config struct, as it's not used in production
    String faucet =
        const String.fromEnvironment("REGTEST_FAUCET", defaultValue: "http://35.189.57.114:8080");

    final data = {'payment_request': invoice};
    final encodedData = json.encode(data);

    final response = await http.post(
      Uri.parse('$faucet/lnd/v1/channels/transactions'),
      headers: <String, String>{'Content-Type': 'application/json'},
      body: encodedData,
    );

    if (response.statusCode == 200) {
      FLog.info(text: response.body);
    } else {
      FLog.error(text: "error: ${response.statusCode} body: ${response.body}");
    }
  }
}
