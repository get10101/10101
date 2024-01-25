import 'dart:async';

import 'package:bitcoin_icons/bitcoin_icons.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:get_10101/common/custom_app_bar.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/application/switch.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/scrollable_safe_area.dart';
import 'package:get_10101/common/secondary_action_button.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/features/wallet/application/faucet_service.dart';
import 'package:get_10101/features/wallet/domain/share_payment_request.dart';
import 'package:get_10101/features/wallet/domain/wallet_type.dart';
import 'package:get_10101/features/wallet/payment_claimed_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:get_10101/logger/logger.dart';
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
  String? description;

  bool _isPayInvoiceButtonDisabled = false;
  SharePaymentRequest? _paymentRequest;
  bool _faucet = false;

  @override
  void initState() {
    super.initState();
    context.read<PaymentClaimedChangeNotifier>().waitForPayment();
    _createPaymentRequest(amount, description)
        .then((paymentRequest) => setState(() => _paymentRequest = paymentRequest));
  }

  String rawInvoice() {
    return _paymentRequest!.bip21Uri;
  }

  String requestTypeName() {
    return "BIP21 payment URI";
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

    return Scaffold(
        body: ScrollableSafeArea(
            child: Container(
      margin: const EdgeInsets.fromLTRB(20, 20.0, 20, 20),
      child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        const TenTenOneAppBar(title: "Receive"),
        Container(
          margin: const EdgeInsets.fromLTRB(0, 10, 0, 0),
          child: GestureDetector(
            onDoubleTap:
                config.network == "regtest" ? () => setState(() => _faucet = !_faucet) : null,
            child: Center(
              child: _faucet
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
                                      .payInvoiceWithFaucet(rawInvoice(), amount)
                                      .catchError((error) {
                                    setState(() => _isPayInvoiceButtonDisabled = false);
                                    showSnackBar(ScaffoldMessenger.of(context), error.toString());
                                  }).then((value) => context.go(WalletScreen.route));
                                },
                          style: ElevatedButton.styleFrom(
                            shape: const RoundedRectangleBorder(
                                borderRadius: BorderRadius.all(Radius.circular(5.0))),
                          ),
                          child: const Text("Pay the invoice with 10101 faucet"),
                        ),
                        const SizedBox(height: 125),
                      ],
                    )
                  : SizedBox.square(
                      dimension: 350,
                      child: QrImageView(
                        data: rawInvoice(),
                        eyeStyle: const QrEyeStyle(
                          eyeShape: QrEyeShape.square,
                          color: Colors.black,
                        ),
                        dataModuleStyle: const QrDataModuleStyle(
                          dataModuleShape: QrDataModuleShape.square,
                          color: Colors.black,
                        ),
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
        ),
        Container(
          margin: const EdgeInsets.fromLTRB(0, 10, 0, 0),
          child: Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Expanded(
                child: SecondaryActionButton(
                  title: "Edit",
                  icon: Icons.edit,
                  onPressed: () => showModalBottomSheet<void>(
                      shape: const RoundedRectangleBorder(
                        borderRadius: BorderRadius.vertical(
                          top: Radius.circular(20),
                        ),
                      ),
                      clipBehavior: Clip.antiAlias,
                      isScrollControlled: true,
                      useRootNavigator: true,
                      context: context,
                      builder: (BuildContext context) => Container(
                            decoration: const BoxDecoration(color: Colors.white),
                            child: Padding(
                              padding: const EdgeInsets.all(20),
                              child: InvoiceDrawerScreen(
                                isInUsd: false,
                                amount: amount?.sats,
                                description: description,
                                isLightning: false,
                                onConfirm: (amt, descr) {
                                  logger.i("Confirming amount $amt");
                                  final satsAmount = Amount(amt);
                                  _createPaymentRequest(satsAmount, descr).then((paymentRequest) {
                                    setState(() {
                                      _paymentRequest = paymentRequest;
                                      amount = satsAmount;
                                      description = descr;
                                    });
                                  });
                                  GoRouter.of(context).pop();
                                },
                              ),
                            ),
                          )),
                ),
              ),
              const SizedBox(width: 10.0),
              Expanded(
                child: SecondaryActionButton(
                  title: "Copy",
                  icon: Icons.copy,
                  onPressed: () {
                    Clipboard.setData(ClipboardData(text: rawInvoice())).then((_) => showSnackBar(
                        ScaffoldMessenger.of(context), "${requestTypeName()} copied to clipboard"));
                  },
                ),
              ),
              const SizedBox(width: 10.0),
              Expanded(
                child: SecondaryActionButton(
                  title: "Share",
                  icon: Icons.share,
                  onPressed: () => Share.share(rawInvoice()),
                ),
              ),
            ],
          ),
        ),
        BitcoinAddress(
          address: _paymentRequest == null ? "" : _paymentRequest!.bip21Uri,
        ),
      ]),
    )));
  }

  Future<SharePaymentRequest> _createPaymentRequest(Amount? amount, String? description) async {
    final completer = Completer<SharePaymentRequest>();

    final walletService = context.read<WalletChangeNotifier>().service;

    final paymentRequest = await walletService.createPaymentRequest(amount, description ?? "");
    completer.complete(paymentRequest);

    return completer.future;
  }
}

class BitcoinAddress extends StatelessWidget {
  final String address;

  const BitcoinAddress({super.key, required this.address});

  @override
  Widget build(BuildContext context) {
    var address = this.address.replaceAll("bitcoin:", '');

    return Container(
        margin: const EdgeInsets.fromLTRB(0, 15, 0, 0),
        decoration: BoxDecoration(
            border: Border.all(color: Colors.grey.shade200),
            borderRadius: BorderRadius.circular(10),
            color: tenTenOnePurple.shade200.withOpacity(0.1)),
        child: GestureDetector(
          onTap: () {
            Clipboard.setData(ClipboardData(text: address)).then((_) {
              showSnackBar(ScaffoldMessenger.of(context), "Address copied to clipboard");
            });
          },
          child: Container(
            padding: const EdgeInsets.all(20),
            child: Opacity(
                opacity: 1.0,
                child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                  Row(mainAxisAlignment: MainAxisAlignment.spaceBetween, children: [
                    const Text("Address", style: TextStyle(fontSize: 18)),
                    IconButton(
                        onPressed: () {
                          Clipboard.setData(ClipboardData(text: address)).then((_) {
                            showSnackBar(
                                ScaffoldMessenger.of(context), "Address copied to clipboard");
                          });
                        },
                        icon: const Icon(
                          Icons.copy,
                          size: 16,
                        ))
                  ]),
                  const SizedBox(height: 5),
                  Row(
                    children: [
                      Expanded(
                        child: Text(
                          address,
                          overflow: TextOverflow.ellipsis,
                          maxLines: 1,
                        ),
                      ),
                    ],
                  )
                ])),
          ),
        ));
  }
}

class LightningUsdpToggle extends StatelessWidget {
  final ValueChanged<bool> updateReceiveUsdp;
  final bool receiveUsdp;
  final Amount? satsAmount;
  final Usd? usdAmount;

  const LightningUsdpToggle(
      {super.key,
      required this.updateReceiveUsdp,
      required this.receiveUsdp,
      required this.satsAmount,
      required this.usdAmount});

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.fromLTRB(0, 15, 0, 0),
      decoration: BoxDecoration(
          border: Border.all(color: Colors.grey.shade200),
          borderRadius: BorderRadius.circular(10),
          color: tenTenOnePurple.shade200.withOpacity(0.1)),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          GestureDetector(
            onTap: () => updateReceiveUsdp(false),
            child: Container(
              padding: const EdgeInsets.all(20),
              child: Opacity(
                  opacity: receiveUsdp ? 0.5 : 1.0,
                  child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                    const Row(mainAxisAlignment: MainAxisAlignment.spaceBetween, children: [
                      Icon(BitcoinIcons.lightning, size: 18),
                      Text("Lightning", style: TextStyle(fontSize: 18))
                    ]),
                    const SizedBox(height: 5),
                    Text(formatSats(satsAmount == null ? Amount.zero() : satsAmount!),
                        textAlign: TextAlign.start),
                  ])),
            ),
          ),
          TenTenOneSwitch(
              value: receiveUsdp,
              isDisabled: false,
              showDisabled: receiveUsdp,
              onChanged: (value) => updateReceiveUsdp(value)),
          GestureDetector(
            onTap: () => updateReceiveUsdp(true),
            child: Container(
              padding: const EdgeInsets.all(20),
              child: Opacity(
                opacity: receiveUsdp ? 1.0 : 0.5,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.end,
                  children: [
                    const Text("USD-P", style: TextStyle(fontSize: 18)),
                    const SizedBox(height: 5),
                    Text(formatUsd(usdAmount == null ? Usd.zero() : usdAmount!),
                        textAlign: TextAlign.end),
                  ],
                ),
              ),
            ),
          )
        ],
      ),
    );
  }
}

class ReceiveActionButton extends StatelessWidget {
  final String title;
  final IconData? icon;
  final VoidCallback onPressed;

  const ReceiveActionButton({
    super.key,
    required this.title,
    this.icon,
    required this.onPressed,
  });

  @override
  Widget build(BuildContext context) {
    return ElevatedButton(
      onPressed: onPressed,
      style: ButtonStyle(
        backgroundColor: MaterialStateProperty.all<Color>(Colors.grey.shade200),
        elevation: MaterialStateProperty.all<double>(1), // this reduces the shade
        padding: MaterialStateProperty.all<EdgeInsetsGeometry>(
          const EdgeInsets.fromLTRB(24, 12, 24, 12),
        ),
        shape: MaterialStateProperty.all<RoundedRectangleBorder>(
          RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(8.0),
          ),
        ),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(
            icon,
            size: 20,
            color: Colors.black,
          ),
          const SizedBox(width: 8),
          Text(title, style: const TextStyle(fontSize: 12, color: Colors.black))
        ],
      ),
    );
  }
}

class DualButtonSelector extends StatefulWidget {
  final String button2Text = "On-chain";
  final VoidCallback onLightningButtonClick;
  final VoidCallback onOnChainButtonClick;
  final bool isLightning;

  const DualButtonSelector({
    super.key,
    required this.onLightningButtonClick,
    required this.onOnChainButtonClick,
    required this.isLightning,
  });

  @override
  DualButtonSelectorState createState() => DualButtonSelectorState();
}

class DualButtonSelectorState extends State<DualButtonSelector> {
  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: BoxDecoration(
        color: Colors.grey.shade200,
        borderRadius: BorderRadius.circular(12.0),
      ),
      padding: const EdgeInsets.fromLTRB(2, 2, 2, 2),
      child: Row(
        children: [
          SelectableButton(
              onPressed: widget.onLightningButtonClick,
              buttonText: 'Lightning',
              isSelected: widget.isLightning,
              selectedColor: tenTenOnePurple,
              icon: BitcoinIcons.lightning),
          const SizedBox(width: 5), // Adjust the spacing between buttons
          SelectableButton(
              onPressed: widget.onOnChainButtonClick,
              buttonText: 'On-chain',
              isSelected: !widget.isLightning,
              selectedColor: Colors.orange,
              icon: BitcoinIcons.bitcoin_circle),
        ],
      ),
    );
  }
}

class SelectableButton extends StatelessWidget {
  final String buttonText;
  final VoidCallback onPressed;
  final bool isSelected;
  final Color selectedColor;
  final IconData? icon;

  const SelectableButton({
    super.key,
    required this.buttonText,
    required this.onPressed,
    required this.isSelected,
    required this.selectedColor,
    required this.icon,
  });

  @override
  Widget build(BuildContext context) {
    return OutlinedButton.icon(
      onPressed: onPressed,
      style: ButtonStyle(
        iconSize: MaterialStateProperty.all<double>(20.0),
        elevation: MaterialStateProperty.all<double>(0), // this reduces the shade
        side: MaterialStateProperty.all(BorderSide(
            width: isSelected ? 1.0 : 0,
            color: isSelected ? selectedColor.withOpacity(1) : Colors.grey.shade300)),
        padding: MaterialStateProperty.all<EdgeInsetsGeometry>(
          const EdgeInsets.fromLTRB(20, 12, 20, 12),
        ),
        shape: MaterialStateProperty.all<RoundedRectangleBorder>(
          RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(8.0),
          ),
        ),
        backgroundColor: isSelected
            ? MaterialStateProperty.all<Color>(selectedColor.withOpacity(0.05))
            : MaterialStateProperty.all<Color>(Colors.grey.shade200),
      ),
      icon: Icon(icon, color: isSelected ? selectedColor.withOpacity(1) : Colors.grey),
      label: Text(buttonText,
          style: TextStyle(color: isSelected ? selectedColor.withOpacity(1) : Colors.grey)),
    );
  }
}

class InvoiceDrawerScreen extends StatefulWidget {
  static const label = "swap";
  final bool isLightning;
  final Function(int, String?) onConfirm;
  final int? amount;
  final String? description;
  final bool isInUsd;

  const InvoiceDrawerScreen(
      {Key? key,
      required this.isLightning,
      required this.onConfirm,
      required this.amount,
      required this.description,
      required this.isInUsd})
      : super(key: key);

  @override
  State<InvoiceDrawerScreen> createState() => _InvoiceDrawerScreen();
}

class _InvoiceDrawerScreen extends State<InvoiceDrawerScreen> {
  int? _amount;
  String? _description;
  bool _isInUsd = false;

  @override
  void initState() {
    super.initState();
    _isInUsd = widget.isInUsd;
    _description = widget.description;
    _amount = widget.amount;
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        Padding(
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
                child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Container(
                    margin: const EdgeInsets.only(top: 5.0, bottom: 10.0, left: 5.0, right: 5.0),
                    child: const Text(
                      "Payment request",
                      style: TextStyle(fontSize: 20, fontWeight: FontWeight.bold),
                    )),
                Container(
                  margin: const EdgeInsets.only(top: 5.0, bottom: 5.0, left: 5.0, right: 5.0),
                  decoration: BoxDecoration(
                      border: Border.all(width: 1, color: Colors.grey.shade300),
                      color: Colors.grey.shade300,
                      borderRadius: BorderRadius.circular(4)),
                  child: InvoiceInputField(
                    onChanged: (value) => {
                      setState(() {
                        _amount = int.parse(value);
                      })
                    },
                    hintText: "Amount",
                    inputFormatters: [
                      FilteringTextInputFormatter.digitsOnly,
                    ],
                    value: _amount == null ? "" : _amount!.toString(),
                    prefixIcon: _isInUsd
                        ? const Icon(FontAwesomeIcons.dollarSign)
                        : const Icon(BitcoinIcons.satoshi_v1),
                    suffixIcon: IconButton(
                      onPressed: () {
                        setState(() {
                          _isInUsd = !_isInUsd;
                        });
                      },
                      icon: const Icon(BitcoinIcons.refresh),
                    ),
                  ),
                ),
                Visibility(
                    visible: widget.isLightning,
                    child: Container(
                      margin: const EdgeInsets.only(top: 5.0, bottom: 5.0, left: 5.0, right: 5.0),
                      decoration: BoxDecoration(
                          border: Border.all(width: 1, color: Colors.grey.shade300),
                          color: Colors.grey.shade300,
                          borderRadius: BorderRadius.circular(4)),
                      child: InvoiceInputField(
                        onChanged: (value) => {
                          setState(() {
                            _description = value;
                          })
                        },
                        hintText: "Description (optional)",
                        inputFormatters: [
                          FilteringTextInputFormatter.singleLineFormatter,
                        ],
                        value: _description ?? "",
                      ),
                    )),
                Container(
                  padding: const EdgeInsets.only(top: 20.0, bottom: 20.0, left: 5.0, right: 5.0),
                  width: double.infinity,
                  child: OutlinedButton(
                    onPressed: () => widget.onConfirm(_amount ?? 0, _description),
                    style: ButtonStyle(
                      fixedSize: MaterialStateProperty.all(const Size(double.infinity, 50)),
                      iconSize: MaterialStateProperty.all<double>(20.0),
                      elevation: MaterialStateProperty.all<double>(0), // this reduces the shade
                      side: MaterialStateProperty.all(
                          const BorderSide(width: 1.0, color: tenTenOnePurple)),
                      padding: MaterialStateProperty.all<EdgeInsetsGeometry>(
                        const EdgeInsets.fromLTRB(20, 12, 20, 12),
                      ),
                      shape: MaterialStateProperty.all<RoundedRectangleBorder>(
                        RoundedRectangleBorder(
                          borderRadius: BorderRadius.circular(8.0),
                        ),
                      ),
                      backgroundColor: MaterialStateProperty.all<Color>(Colors.transparent),
                    ),
                    child: const Text("Continue"),
                  ),
                )
              ],
            )),
          ),
        )
      ],
    );
  }
}

class InvoiceInputField extends StatelessWidget {
  final ValueChanged onChanged;
  final String hintText;
  final List<TextInputFormatter>? inputFormatters;
  final String value;
  final Widget? prefixIcon;
  final Widget? suffixIcon;

  const InvoiceInputField({
    super.key,
    required this.onChanged,
    required this.hintText,
    required this.inputFormatters,
    required this.value,
    this.prefixIcon,
    this.suffixIcon,
  });

  @override
  Widget build(BuildContext context) {
    return TextFormField(
      initialValue: value,
      decoration: InputDecoration(
          border: InputBorder.none,
          hintText: hintText,
          labelStyle: const TextStyle(color: Colors.black87),
          filled: true,
          fillColor: Colors.grey[50],
          errorStyle: TextStyle(
            color: Colors.red[900],
          ),
          prefixIcon: prefixIcon,
          suffixIcon: suffixIcon),
      style: const TextStyle(
        fontSize: 16,
        fontWeight: FontWeight.w400,
      ),
      inputFormatters: inputFormatters,
      onChanged: onChanged,
    );
  }
}
