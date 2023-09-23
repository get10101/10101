import 'dart:convert';
import 'package:f_logs/f_logs.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/modal_bottom_sheet_info.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/common/value_data_row.dart';
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
import 'package:get_10101/features/wallet/domain/share_invoice.dart';

class ShareInvoiceScreen extends StatefulWidget {
  static const route = "${WalletScreen.route}/${CreateInvoiceScreen.subRouteName}/$subRouteName";
  static const subRouteName = "share_invoice";
  final ShareInvoice invoice;

  const ShareInvoiceScreen({super.key, required this.invoice});

  @override
  State<ShareInvoiceScreen> createState() => _ShareInvoiceScreenState();
}

class _ShareInvoiceScreenState extends State<ShareInvoiceScreen> {
  bool _isPayInvoiceButtonDisabled = false;

  @override
  Widget build(BuildContext context) {
    WalletInfo info = context.watch<WalletChangeNotifier>().walletInfo;
    final bridge.Config config = context.read<bridge.Config>();

    FLog.debug(text: "Refresh receive screen: ${formatSats(info.balances.onChain)}");

    const EdgeInsets buttonSpacing = EdgeInsets.symmetric(vertical: 8.0, horizontal: 24.0);

    const qrWidth = 200.0;
    const qrPadding = 5.0;
    const infoButtonRadius = ModalBottomSheetInfo.buttonRadius;
    const infoButtonPadding = 5.0;

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
                  padding: EdgeInsets.only(top: 25.0, bottom: 30.0),
                  child: Text(
                    "Share payment request",
                    style: TextStyle(fontSize: 18.0, fontWeight: FontWeight.bold),
                  ),
                ),
                Expanded(
                  child: Center(
                    child: QrImageView(
                      data: widget.invoice.rawInvoice,
                      version: QrVersions.auto,
                      padding: const EdgeInsets.all(qrPadding),
                    ),
                  ),
                ),
                const SizedBox(height: 10),
                Center(
                    child: SizedBox(
                        // Size of the qr image minus padding
                        width: qrWidth - 2 * qrPadding,
                        child: ValueDataRow(
                          type: ValueType.amount,
                          value: widget.invoice.invoiceAmount,
                          label: 'Amount',
                        ))),
                if (widget.invoice.channelOpenFee != null)
                  Padding(
                    // Set in by size of info button on the right
                    padding: const EdgeInsets.only(left: infoButtonRadius * 2),
                    child: SizedBox(
                      width: qrWidth - 2 * qrPadding + infoButtonRadius * 2,
                      child: Row(
                        children: [
                          Expanded(
                              child: ValueDataRow(
                                  type: ValueType.amount,
                                  value: widget.invoice.channelOpenFee,
                                  label: "Fee Estimate")),
                          ModalBottomSheetInfo(
                              closeButtonText: "Back to Share Invoice",
                              infoButtonPadding: const EdgeInsets.all(infoButtonPadding),
                              child: Column(
                                children: [
                                  Center(
                                    child: Text("Understanding Fees",
                                        style: Theme.of(context).textTheme.headlineSmall),
                                  ),
                                  const SizedBox(height: 10),
                                  Text(
                                      "Upon receiving your first payment, the 10101 LSP will open a Lightning channel with you.\n"
                                      "To cover the costs for opening the channel, a transaction fee (estimated ${formatSats(widget.invoice.channelOpenFee!)}) will be collected after the channel is opened.\n"
                                      "The fee estimate is based on a transaction weight with two inputs and the current estimated fee rate."),
                                ],
                              )),
                        ],
                      ),
                    ),
                  ),
                const SizedBox(height: 10)
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
                        final messenger = ScaffoldMessenger.of(context);
                        try {
                          await payInvoiceWithLndFaucet(widget.invoice.rawInvoice);
                          // Pop both create invoice screen and share invoice screen
                          router.pop();
                          router.pop();
                        } catch (error) {
                          showSnackBar(messenger, error.toString());
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
                        final messenger = ScaffoldMessenger.of(context);
                        try {
                          await payInvoiceWithMakerFaucet(widget.invoice.rawInvoice);
                          // Pop both create invoice screen and share invoice screen
                          router.pop();
                          router.pop();
                        } catch (error) {
                          showSnackBar(messenger, error.toString());
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
                child: const Text("Pay the invoice with maker faucet"),
              ),
            ),
            Row(children: [
              Flexible(
                flex: 1,
                child: Padding(
                  padding: buttonSpacing,
                  child: OutlinedButton(
                    onPressed: () {
                      Clipboard.setData(ClipboardData(text: widget.invoice.rawInvoice)).then((_) {
                        showSnackBar(ScaffoldMessenger.of(context), "Invoice copied to clipboard");
                      });
                    },
                    style: ElevatedButton.styleFrom(
                      shape: const RoundedRectangleBorder(
                          borderRadius: BorderRadius.all(Radius.circular(5.0))),
                    ),
                    child: const Row(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
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
                    onPressed: () => Share.share(widget.invoice.rawInvoice),
                    style: ElevatedButton.styleFrom(
                      shape: const RoundedRectangleBorder(
                          borderRadius: BorderRadius.all(Radius.circular(5.0))),
                    ),
                    child: const Row(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
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
                onPressed: _isPayInvoiceButtonDisabled
                    ? null
                    : () {
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
  Future<void> payInvoiceWithLndFaucet(String invoice) async {
    // Default to the faucet on the 10101 server, but allow to override it
    // locally if needed for dev testing
    // It's not populated in Config struct, as it's not used in production
    String faucet =
        const String.fromEnvironment("REGTEST_FAUCET", defaultValue: "http://34.32.0.52:8080");

    final data = {'payment_request': invoice};
    final encodedData = json.encode(data);

    final response = await http.post(
      Uri.parse('$faucet/lnd/v1/channels/transactions'),
      headers: <String, String>{'Content-Type': 'application/json'},
      body: encodedData,
    );

    if (response.statusCode != 200 || !response.body.contains('"payment_error":""')) {
      throw Exception("Payment failed: Received ${response.statusCode} ${response.body}");
    } else {
      FLog.info(text: "Paying invoice succeeded: ${response.body}");
    }
  }

  // Pay the generated invoice with maker faucet
  Future<void> payInvoiceWithMakerFaucet(String invoice) async {
    // Default to the faucet on the 10101 server, but allow to override it
    // locally if needed for dev testing
    // It's not populated in Config struct, as it's not used in production
    String faucet = const String.fromEnvironment("REGTEST_MAKER_FAUCET",
        defaultValue: "http://34.32.0.52:80/maker/faucet");

    final response = await http.post(
      Uri.parse('$faucet/$invoice'),
    );

    FLog.info(text: "Response ${response.body}${response.statusCode}");

    if (response.statusCode != 200) {
      throw Exception("Payment failed: Received ${response.statusCode}. ${response.body}");
    } else {
      FLog.info(text: "Paying invoice succeeded: ${response.body}");
    }
  }
}
