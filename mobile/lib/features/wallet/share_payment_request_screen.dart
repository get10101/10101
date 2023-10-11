import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_svg/svg.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/wallet/application/faucet_service.dart';
import 'package:get_10101/features/wallet/domain/share_payment_request.dart';
import 'package:get_10101/features/wallet/payment_claimed_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:get_10101/features/wallet/wallet_theme.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';
import 'package:qr_flutter/qr_flutter.dart';
import 'package:share_plus/share_plus.dart';

class SharePaymentRequestScreen extends StatefulWidget {
  static const route = "${WalletScreen.route}/$subRouteName";
  static const subRouteName = "share_payment_request";
  final SharePaymentRequest request;

  const SharePaymentRequestScreen({super.key, required this.request});

  @override
  State<SharePaymentRequestScreen> createState() => _SharePaymentRequestScreenState();
}

class _SharePaymentRequestScreenState extends State<SharePaymentRequestScreen> {
  bool _isPayInvoiceButtonDisabled = false;
  bool _isLightning = true;

  @override
  void initState() {
    super.initState();
    context.read<PaymentClaimedChangeNotifier>().waitForPayment();
  }

  String rawInvoice() {
    return _isLightning ? widget.request.lightningInvoice : widget.request.bip21Uri;
  }

  String requestTypeName() {
    return _isLightning ? "Invoice" : "BIP21 payment URI";
  }

  @override
  Widget build(BuildContext context) {
    final bridge.Config config = context.read<bridge.Config>();

    if (context.watch<PaymentClaimedChangeNotifier>().isClaimed()) {
      // routing is not allowed during building a widget, hence we need to register the route navigation after the widget has been build.
      WidgetsBinding.instance.addPostFrameCallback((_) {
        logger.d("Payment received!");
        GoRouter.of(context).pop();
      });
    }

    const EdgeInsets buttonSpacing = EdgeInsets.symmetric(vertical: 8.0, horizontal: 24.0);

    const qrWidth = 200.0;
    const qrPadding = 5.0;

    WalletTheme theme = Theme.of(context).extension<WalletTheme>()!;
    HSLColor hsl = HSLColor.fromColor(theme.lightning);
    Color lightningColor = hsl.withLightness(hsl.lightness - 0.17).toColor();

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
                  padding: EdgeInsets.only(top: 25.0, bottom: 15.0),
                  child: Text(
                    "Share payment request",
                    style: TextStyle(fontSize: 18.0, fontWeight: FontWeight.bold),
                  ),
                ),
                SegmentedButton(
                  segments: [
                    ButtonSegment(
                        value: true,
                        label: const Text('Lightning'),
                        icon: SizedBox(
                            width: 30,
                            child: SvgPicture.asset("assets/Lightning_logo.svg",
                                colorFilter: ColorFilter.mode(lightningColor, BlendMode.srcIn)))),
                    ButtonSegment(
                        value: false,
                        label: const Text('Bitcoin'),
                        icon: SizedBox(
                            width: 30, child: SvgPicture.asset("assets/Bitcoin_logo.svg"))),
                  ],
                  style: const ButtonStyle(
                      side: MaterialStatePropertyAll(BorderSide(width: 1.0, color: Colors.grey))),
                  selected: {_isLightning},
                  showSelectedIcon: false,
                  onSelectionChanged: (newSelection) {
                    logger.i("new: $newSelection");
                    setState(() => _isLightning = newSelection.first);
                  },
                ),
                Expanded(
                  child: Center(
                    child: QrImageView(
                      data: rawInvoice(),
                      version: QrVersions.auto,
                      padding: const EdgeInsets.all(qrPadding),
                    ),
                  ),
                ),
                if (widget.request.amount != null) ...[
                  const SizedBox(height: 10),
                  Center(
                      child: SizedBox(
                          // Size of the qr image minus padding
                          width: qrWidth - 2 * qrPadding,
                          child: ValueDataRow(
                            type: ValueType.amount,
                            value: widget.request.amount!,
                            label: 'Amount',
                          ))),
                ],
                const SizedBox(height: 10)
              ]),
            ),
            // Faucet button, only available if we are on regtest
            // TODO(on-chain): allow paying on-chain via faucet
            Visibility(
              visible: config.network == "regtest" && _isLightning,
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
                          await faucetService.payInvoiceWithLndFaucet(rawInvoice());
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
              visible: config.network == "regtest" && _isLightning,
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
                          await faucetService.payInvoiceWithMakerFaucet(rawInvoice());
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
                      Clipboard.setData(ClipboardData(text: rawInvoice())).then((_) {
                        showSnackBar(ScaffoldMessenger.of(context),
                            "${requestTypeName()} copied to clipboard");
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
                    onPressed: () => Share.share(rawInvoice()),
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
