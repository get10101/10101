import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/amount_text_input_form_field.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/scrollable_safe_area.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/features/wallet/application/faucet_service.dart';
import 'package:get_10101/features/wallet/domain/share_payment_request.dart';
import 'package:get_10101/features/wallet/domain/wallet_type.dart';
import 'package:get_10101/features/wallet/payment_claimed_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:qr_flutter/qr_flutter.dart';
import 'package:share_plus/share_plus.dart';

class ReceiveScreen extends StatefulWidget {
  static const route = "${WalletScreen.route}/$subRouteName";
  static const subRouteName = "receive";

  final WalletType walletType;

  const ReceiveScreen({super.key, this.walletType = WalletType.lightning});

  @override
  State<ReceiveScreen> createState() => _ReceiveScreenState();
}

class _ReceiveScreenState extends State<ReceiveScreen> {
  Amount? amount;

  bool _isPayInvoiceButtonDisabled = false;
  late bool _isLightning;
  SharePaymentRequest? _paymentRequest;
  bool _faucet = false;

  @override
  void initState() {
    super.initState();
    context.read<PaymentClaimedChangeNotifier>().waitForPayment();
    _createPaymentRequest(amount)
        .then((paymentRequest) => setState(() => _paymentRequest = paymentRequest));
    _isLightning = widget.walletType == WalletType.lightning;
  }

  String rawInvoice() {
    return _isLightning ? _paymentRequest!.lightningInvoice : _paymentRequest!.bip21Uri;
  }

  String requestTypeName() {
    return _isLightning ? "Invoice" : "BIP21 payment URI";
  }

  @override
  Widget build(BuildContext context) {
    final bridge.Config config = context.read<bridge.Config>();

    if (_paymentRequest == null) {
      return Scaffold(
          appBar: AppBar(title: const Text("Receive funds")),
          body: const Center(
              child: SizedBox(width: 20, height: 20, child: CircularProgressIndicator())));
    }

    final isPaymentClaimed = context.watch<PaymentClaimedChangeNotifier>().isClaimed();
    if (isPaymentClaimed) {
      // routing is not allowed during building a widget, hence we need to register the route navigation after the widget has been build.
      WidgetsBinding.instance.addPostFrameCallback((_) {
        context
            .read<WalletChangeNotifier>()
            .refreshLightningWallet()
            .then((value) => GoRouter.of(context).pop());
      });
    }

    final lightningColor = _isLightning ? tenTenOnePurple : Colors.grey;
    final bitcoinColor = !_isLightning ? tenTenOnePurple : Colors.grey;

    return Scaffold(
        appBar: AppBar(title: const Text("Receive funds")),
        body: ScrollableSafeArea(
            child: Container(
          padding: const EdgeInsets.all(20.0),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              const SizedBox(height: 5),
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceEvenly,
                children: [
                  Expanded(
                    child: OutlinedButton(
                        onPressed: () => setState(() => _isLightning = true),
                        style: OutlinedButton.styleFrom(
                            minimumSize: const Size(20, 50),
                            side: BorderSide(color: lightningColor),
                            shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
                            backgroundColor: Colors.white),
                        child: Row(
                          mainAxisAlignment: MainAxisAlignment.spaceBetween,
                          children: [
                            Text("Lightning",
                                style: TextStyle(color: lightningColor, fontSize: 16)),
                            Icon(Icons.bolt, color: lightningColor),
                          ],
                        )),
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                      child: OutlinedButton(
                          onPressed: () => setState(() => _isLightning = false),
                          style: OutlinedButton.styleFrom(
                            minimumSize: const Size(20, 50),
                            side: BorderSide(color: bitcoinColor),
                            backgroundColor: Colors.white,
                            shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
                          ),
                          child: Row(
                            mainAxisAlignment: MainAxisAlignment.spaceBetween,
                            children: [
                              Text("Bitcoin", style: TextStyle(color: bitcoinColor, fontSize: 16)),
                              Icon(
                                Icons.currency_bitcoin,
                                color: bitcoinColor,
                              ),
                            ],
                          )))
                ],
              ),
              const SizedBox(height: 15),
              GestureDetector(
                onDoubleTap: config.network == "regtest" && _isLightning
                    ? () => setState(() => _faucet = !_faucet)
                    : null,
                child: Center(
                  child: _faucet && _isLightning
                      ? Column(
                          children: [
                            const SizedBox(height: 125),
                            OutlinedButton(
                              onPressed: _isPayInvoiceButtonDisabled
                                  ? null
                                  : () async {
                                      setState(() => _isPayInvoiceButtonDisabled = true);
                                      final faucetService = context.read<FaucetService>();
                                      faucetService
                                          .payInvoiceWithLndFaucet(rawInvoice())
                                          .catchError((error) {
                                        setState(() => _isPayInvoiceButtonDisabled = false);
                                        showSnackBar(
                                            ScaffoldMessenger.of(context), error.toString());
                                      });
                                    },
                              style: ElevatedButton.styleFrom(
                                shape: const RoundedRectangleBorder(
                                    borderRadius: BorderRadius.all(Radius.circular(5.0))),
                              ),
                              child: const Text("Pay the invoice with 10101 faucet"),
                            ),
                            OutlinedButton(
                              onPressed: _isPayInvoiceButtonDisabled
                                  ? null
                                  : () async {
                                      setState(() => _isPayInvoiceButtonDisabled = true);
                                      final faucetService = context.read<FaucetService>();
                                      faucetService
                                          .payInvoiceWithMakerFaucet(rawInvoice())
                                          .catchError((error) {
                                        setState(() => _isPayInvoiceButtonDisabled = false);
                                        showSnackBar(
                                            ScaffoldMessenger.of(context), error.toString());
                                      });
                                    },
                              style: ElevatedButton.styleFrom(
                                shape: const RoundedRectangleBorder(
                                    borderRadius: BorderRadius.all(Radius.circular(5.0))),
                              ),
                              child: const Text("Pay the invoice with 10101 maker"),
                            ),
                            const SizedBox(height: 125),
                          ],
                        )
                      : SizedBox(
                          width: 300,
                          height: 300,
                          child: QrImageView(
                            data: rawInvoice(),
                            embeddedImage:
                                const AssetImage('assets/10101_logo_icon_white_background.png'),
                            embeddedImageStyle: const QrEmbeddedImageStyle(
                              size: Size(50, 50),
                            ),
                            version: QrVersions.auto,
                            padding: const EdgeInsets.all(5),
                          ),
                        ),
                ),
              ),
              Row(mainAxisAlignment: MainAxisAlignment.spaceBetween, children: [
                OutlinedButton(
                  onPressed: () {
                    Clipboard.setData(ClipboardData(text: rawInvoice())).then((_) => showSnackBar(
                        ScaffoldMessenger.of(context), "${requestTypeName()} copied to clipboard"));
                  },
                  style: ElevatedButton.styleFrom(
                    minimumSize: const Size(150, 40),
                    side: const BorderSide(color: tenTenOnePurple),
                    shape: const RoundedRectangleBorder(
                        borderRadius: BorderRadius.all(Radius.circular(5.0))),
                  ),
                  child: const Row(
                    children: [
                      Icon(Icons.copy, size: 15),
                      SizedBox(width: 10),
                      Text("Copy"),
                    ],
                  ),
                ),
                OutlinedButton(
                    onPressed: () => Share.share(rawInvoice()),
                    style: ElevatedButton.styleFrom(
                      minimumSize: const Size(150, 40),
                      side: const BorderSide(color: tenTenOnePurple),
                      shape: const RoundedRectangleBorder(
                          borderRadius: BorderRadius.all(Radius.circular(5.0))),
                    ),
                    child: const Row(
                      children: [
                        Icon(Icons.share, size: 15),
                        SizedBox(width: 10),
                        Text(
                          "Share",
                          style: TextStyle(color: tenTenOnePurple, fontSize: 16),
                        ),
                      ],
                    ))
              ]),
              const SizedBox(height: 15),
              OutlinedButton(
                  onPressed: () => showEnterAmountModal(context, amount, (amt) {
                        _createPaymentRequest(amt).then((paymentRequest) {
                          setState(() {
                            _paymentRequest = paymentRequest;
                            amount = amt;
                          });
                        });
                      }),
                  style: OutlinedButton.styleFrom(
                    minimumSize: const Size(20, 50),
                    backgroundColor: Colors.white,
                    shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
                  ),
                  child: Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      Text(
                        amount != null ? formatSats(amount!) : "Set amount",
                        style: const TextStyle(color: Colors.black87, fontSize: 16),
                      ),
                      const Icon(Icons.edit, size: 20)
                    ],
                  )),
              Expanded(child: Container()),
              ElevatedButton(
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
            ],
          ),
        )));
  }

  Future<SharePaymentRequest> _createPaymentRequest(Amount? amount) async {
    var completer = Completer<SharePaymentRequest>();

    final walletService = context.read<WalletChangeNotifier>().service;

    final paymentRequest = await walletService.createPaymentRequest(amount);
    completer.complete(paymentRequest);

    return completer.future;
  }
}

void showEnterAmountModal(BuildContext context, Amount? amount, Function onSetAmount) {
  showModalBottomSheet<void>(
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(
          top: Radius.circular(20),
        ),
      ),
      clipBehavior: Clip.antiAlias,
      isScrollControlled: true,
      useRootNavigator: true,
      context: context,
      builder: (BuildContext context) {
        return SafeArea(
            child: Padding(
                padding: EdgeInsets.only(bottom: MediaQuery.of(context).viewInsets.bottom),
                // the GestureDetector ensures that we can close the keyboard by tapping into the modal
                child: GestureDetector(
                  onTap: () {
                    FocusScopeNode currentFocus = FocusScope.of(context);

                    if (!currentFocus.hasPrimaryFocus) {
                      currentFocus.unfocus();
                    }
                  },
                  child: SingleChildScrollView(
                    child: SizedBox(
                      // TODO: Find a way to make height dynamic depending on the children size
                      // This is needed because otherwise the keyboard does not push the sheet up correctly
                      height: 200,
                      child: EnterAmountModal(amount: amount, onSetAmount: onSetAmount),
                    ),
                  ),
                )));
      });
}

class EnterAmountModal extends StatefulWidget {
  final Amount? amount;
  final Function onSetAmount;

  const EnterAmountModal({super.key, this.amount, required this.onSetAmount});

  @override
  State<EnterAmountModal> createState() => _EnterAmountModalState();
}

class _EnterAmountModalState extends State<EnterAmountModal> {
  Amount? amount;

  @override
  void initState() {
    super.initState();
    amount = widget.amount;
  }

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(left: 20.0, top: 30.0, right: 20.0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          AmountInputField(
            value: widget.amount ?? Amount.zero(),
            hint: "e.g. ${formatSats(Amount(50000))}",
            label: "Amount",
            onChanged: (value) {
              if (value.isEmpty) {
                amount = null;
              }
              amount = Amount.parseAmount(value);
            },
          ),
          const SizedBox(height: 20),
          ElevatedButton(
              onPressed: () {
                widget.onSetAmount(amount);
                GoRouter.of(context).pop();
              },
              child: const Text("Set Amount", style: TextStyle(fontSize: 16)))
        ],
      ),
    );
  }
}
