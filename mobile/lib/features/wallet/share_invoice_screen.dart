import 'package:f_logs/f_logs.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/wallet/application/faucet_service.dart';
import 'package:get_10101/features/wallet/domain/share_invoice.dart';
import 'package:get_10101/features/wallet/payment_claimed_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';
import 'package:qr_flutter/qr_flutter.dart';
import 'package:share_plus/share_plus.dart';

class ShareInvoiceScreen extends StatefulWidget {
  static const route = "${WalletScreen.route}/$subRouteName";
  static const subRouteName = "share_invoice";
  final ShareInvoice invoice;

  const ShareInvoiceScreen({super.key, required this.invoice});

  @override
  State<ShareInvoiceScreen> createState() => _ShareInvoiceScreenState();
}

class _ShareInvoiceScreenState extends State<ShareInvoiceScreen> {
  bool _isPayInvoiceButtonDisabled = false;

  @override
  void initState() {
    super.initState();
    context.read<PaymentClaimedChangeNotifier>().waitForPayment();
  }

  @override
  Widget build(BuildContext context) {
    final bridge.Config config = context.read<bridge.Config>();

    if (context.watch<PaymentClaimedChangeNotifier>().isClaimed()) {
      // routing is not allowed during building a widget, hence we need to register the route navigation after the widget has been build.
      WidgetsBinding.instance.addPostFrameCallback((_) {
        FLog.debug(text: "Payment received!");
        GoRouter.of(context).pop();
      });
    }

    const EdgeInsets buttonSpacing = EdgeInsets.symmetric(vertical: 8.0, horizontal: 24.0);

    const qrWidth = 200.0;
    const qrPadding = 5.0;

    return Scaffold(
      appBar: AppBar(title: const Text("Share payment request")),
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
                const SizedBox(height: 10)
              ]),
            ),
            // Faucet button, only available if we are on regtest
            // TODO(on-chain): allow paying on-chain via faucet
            Visibility(
              visible: config.network == "regtest" && widget.invoice.isLightning,
              child: OutlinedButton(
                onPressed: _isPayInvoiceButtonDisabled
                    ? null
                    : () async {
                        setState(() {
                          _isPayInvoiceButtonDisabled = true;
                        });

                        final messenger = ScaffoldMessenger.of(context);
                        try {
                          final faucetService = context.read<FaucetService>();
                          await faucetService.payInvoiceWithLndFaucet(widget.invoice.rawInvoice);
                        } catch (error) {
                          showSnackBar(messenger, error.toString());
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
              // TODO(on-chain): allow paying on-chain via faucet
              visible: config.network == "regtest" && widget.invoice.isLightning,
              child: OutlinedButton(
                onPressed: _isPayInvoiceButtonDisabled
                    ? null
                    : () async {
                        setState(() {
                          _isPayInvoiceButtonDisabled = true;
                        });

                        final messenger = ScaffoldMessenger.of(context);
                        try {
                          final faucetService = context.read<FaucetService>();
                          await faucetService.payInvoiceWithMakerFaucet(widget.invoice.rawInvoice);
                        } catch (error) {
                          showSnackBar(messenger, error.toString());
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
}
