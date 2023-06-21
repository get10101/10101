import 'dart:convert';
import 'dart:developer';

import 'package:f_logs/f_logs.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/snack_bar.dart';
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
import 'package:get_10101/ffi.dart' as rust;

import 'application/wallet_service.dart';

class ShareInvoiceScreen extends StatefulWidget {
  static const route = "${WalletScreen.route}/${CreateInvoiceScreen.subRouteName}/$subRouteName";
  static const subRouteName = "share_invoice";
  final WalletService walletService;
  final String invoice;

  const ShareInvoiceScreen(
      {super.key, this.walletService = const WalletService(), required this.invoice});

  @override
  State<ShareInvoiceScreen> createState() => _ShareInvoiceScreenState();
}

class _ShareInvoiceScreenState extends State<ShareInvoiceScreen> {
  bool _isPayInvoiceButtonDisabled = false;

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
                  child: Center(
                    child: QrImage(
                      data: widget.invoice,
                      version: QrVersions.auto,
                      size: 200.0,
                    ),
                  ),
                ),
              ]),
            ),
            // Faucet button, only available if we are on regtest
            Visibility(
              visible: config.network == "regtest",
              child: OutlinedButton(
                onPressed: _isPayInvoiceButtonDisabled
                    ? null
                    : () async {
                        setState(() {
                          _isPayInvoiceButtonDisabled = true;
                        });

                        final router = GoRouter.of(context);
                        const localCoordinatorPubkey =
                            "02dd6abec97f9a748bf76ad502b004ce05d1b2d1f43a9e76bd7d85e767ffb022c9";
                        try {
                          if (config.coordinatorPubkey == localCoordinatorPubkey) {
                            await payInvoiceWithFaucet(widget.invoice);
                          } else {
                            // For remote coordinator, we open the channel
                            // directly with the coordinator as it's less error-prone
                            await openCoordinatorChannel(config.host);
                          }
                          // Pop both create invoice screen and share invoice screen
                          router.pop();
                          router.pop();
                        } catch (error) {
                          showSnackBar(context, error.toString());
                        } finally {
                          setState(() {
                            _isPayInvoiceButtonDisabled = false;
                          });
                        }
                      },
                style: ElevatedButton.styleFrom(
                  shape: const RoundedRectangleBorder(
                      borderRadius: BorderRadius.all(Radius.circular(5.0))),
                ),
                child: const Text("Pay the invoice with 10101 faucet"),
              ),
            ),
            Row(children: [
              Flexible(
                flex: 1,
                child: Padding(
                  padding: buttonSpacing,
                  child: OutlinedButton(
                    onPressed: () {
                      Clipboard.setData(ClipboardData(text: widget.invoice)).then((_) {
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
                    onPressed: () => Share.share(widget.invoice),
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

    if (response.statusCode != 200 || response.body.contains("payment_error")) {
      throw Exception("Payment failed: Received ${response.statusCode} ${response.body}");
    } else {
      FLog.info(text: "Paying invoice succeeded: ${response.body}");
    }
  }

  // Open channel directly between coordinator and app.
  //
  // Just for regtest.
  Future<void> openCoordinatorChannel(String coordinatorHost) async {
    int coordinatorPort = const int.fromEnvironment("COORDINATOR_PORT_HTTP", defaultValue: 8000);
    var coordinator = 'http://$coordinatorHost:$coordinatorPort';

    final requestBody = {
      'target': {'pubkey': rust.api.getNodeId()},
      'local_balance': 200000,
      'remote_balance': 100000,
      'is_public': false
    };
    final jsonString = json.encode(requestBody).toString();

    FLog.info(text: jsonString);
    FLog.info(text: coordinator);

    final response = await http.post(
      Uri.parse('$coordinator/api/channels'),
      headers: <String, String>{'Content-Type': 'application/json'},
      body: jsonString,
    );

    if (response.statusCode != 200 || response.body.contains("payment_error")) {
      throw Exception(
          "Failed to open channel with coordinator: Received ${response.statusCode} ${response.body}");
    } else {
      FLog.info(text: "Initiating channel open with coordinator succeeded: ${response.body}");
    }
  }
}
