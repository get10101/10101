import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/custom_app_bar.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/scrollable_safe_area.dart';
import 'package:get_10101/features/wallet/application/util.dart';
import 'package:get_10101/features/wallet/application/wallet_service.dart';
import 'package:get_10101/features/wallet/domain/confirmation_target.dart';
import 'package:get_10101/features/wallet/domain/destination.dart';
import 'package:get_10101/features/wallet/domain/fee.dart';
import 'package:get_10101/features/wallet/domain/fee_estimate.dart';
import 'package:get_10101/features/wallet/send/confirm_payment_modal.dart';
import 'package:get_10101/features/wallet/send/fee_picker.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:provider/provider.dart';

class SendOnChainScreen extends StatefulWidget {
  static const route = "${WalletScreen.route}/$subRouteName";
  static const subRouteName = "send-onchain";

  final OnChainAddress destination;

  const SendOnChainScreen({super.key, required this.destination});

  @override
  State<SendOnChainScreen> createState() => _SendOnChainScreenState();
}

class _SendOnChainScreenState extends State<SendOnChainScreen> {
  final _formKey = GlobalKey<FormState>();

  // null = max
  Amount? _amount = Amount.zero();
  FeeConfig _feeConfig = PriorityFee(ConfirmationTarget.normal);
  FeeEstimation? _customFee;
  Map<ConfirmationTarget, FeeEstimation>? _feeEstimates;
  late WalletService _walletService;

  final TextEditingController _controller = TextEditingController();

  @override
  void initState() {
    super.initState();
    _walletService = context.read<WalletChangeNotifier>().service;

    _walletService
        .calculateFeesForOnChain(widget.destination.address)
        .then((fees) => setState(() => _feeEstimates = fees));

    setState(() {
      Amount amt = widget.destination.amount;
      _amount = amt;
      _controller.text = amt.formatted();
    });
  }

  @override
  void dispose() {
    super.dispose();
    _controller.dispose();
  }

  Future<void> calculateCustomFee(CustomFeeRate feeRate) async {
    FeeEstimation? feeEstimation =
        await _walletService.calculateCustomFee(widget.destination.address, feeRate);

    setState(() {
      _customFee = feeEstimation;
    });
  }

  FeeEstimation? currentFee() {
    return switch (_feeConfig) {
      PriorityFee() => _feeEstimates?[(_feeConfig as PriorityFee).priority],
      CustomFeeRate() => _customFee,
    };
  }

  @override
  Widget build(BuildContext context) {
    final walletInfo = context.read<WalletChangeNotifier>().walletInfo;
    final balance = walletInfo.balances.onChain;

    return GestureDetector(
      onTap: () => FocusManager.instance.primaryFocus?.unfocus(),
      child: Scaffold(
        resizeToAvoidBottomInset: true,
        body: ScrollableSafeArea(
          child: Form(
            key: _formKey,
            autovalidateMode: AutovalidateMode.always,
            child: SafeArea(
              child: GestureDetector(
                onTap: () => FocusManager.instance.primaryFocus?.unfocus(),
                child: Container(
                  margin: const EdgeInsets.all(20.0),
                  child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                    const TenTenOneAppBar(title: "Send"),
                    const SizedBox(
                      height: 20,
                    ),
                    Container(
                      padding: const EdgeInsets.all(20),
                      decoration: BoxDecoration(
                          border: Border.all(color: Colors.grey.shade200),
                          borderRadius: BorderRadius.circular(10),
                          color: Colors.orange.shade300.withOpacity(0.1)),
                      child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                        const Text(
                          "Send to:",
                          style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold),
                          textAlign: TextAlign.start,
                        ),
                        const SizedBox(height: 2),
                        Row(mainAxisAlignment: MainAxisAlignment.spaceBetween, children: [
                          Text(truncateWithEllipsis(18, widget.destination.raw),
                              overflow: TextOverflow.ellipsis,
                              style: const TextStyle(fontSize: 16)),
                          Container(
                            padding: const EdgeInsets.only(left: 10, right: 10, top: 5, bottom: 5),
                            decoration: BoxDecoration(
                              color: Colors.orange,
                              border: Border.all(color: Colors.grey.shade200),
                              borderRadius: BorderRadius.circular(20),
                            ),
                            child: const Row(
                              mainAxisAlignment: MainAxisAlignment.spaceBetween,
                              children: [
                                Icon(Icons.currency_bitcoin, size: 14, color: Colors.white),
                                SizedBox(width: 5),
                                Text("On-Chain",
                                    style: TextStyle(fontSize: 14, color: Colors.white))
                              ],
                            ),
                          )
                        ])
                      ]),
                    ),
                    const SizedBox(height: 25),
                    const Text(
                      "Amount",
                      textAlign: TextAlign.center,
                      style: TextStyle(fontSize: 14, color: Colors.grey),
                    ),
                    const SizedBox(height: 10),
                    Container(
                        margin: const EdgeInsets.only(left: 40, right: 40),
                        child: FormField(
                          validator: (_) {
                            final amount = _amount;

                            // This corresponds to sending the max amount.
                            if (amount == null) {
                              return null;
                            }

                            if (amount.sats < 0) {
                              return "Amount cannot be negative";
                            }

                            FeeEstimation? fee = currentFee();
                            if (fee == null) {
                              return "Select a fee";
                            }

                            if (amount.sats + fee.total.sats > balance.sats) {
                              return "Not enough funds";
                            }

                            if (amount.sats == 0) {
                              return sendAmountIsZero;
                            }

                            return null;
                          },
                          builder: (FormFieldState<Object> formFieldState) {
                            return Column(
                              children: [
                                TextField(
                                  keyboardType: TextInputType.number,
                                  textAlign: TextAlign.center,
                                  decoration: const InputDecoration(
                                      enabledBorder: InputBorder.none,
                                      border: InputBorder.none,
                                      errorBorder: InputBorder.none,
                                      suffix: Text(
                                        "sats",
                                        style: TextStyle(fontSize: 16),
                                      )),
                                  style: const TextStyle(fontSize: 40),
                                  textAlignVertical: TextAlignVertical.center,
                                  enabled: _amount != null,
                                  controller: _controller,
                                  onChanged: (value) {
                                    Amount amt = Amount.parseAmount(value);
                                    setState(() {
                                      _amount = amt;
                                      _controller.text = amt.formatted();
                                    });

                                    _walletService
                                        .calculateFeesForOnChain(widget.destination.address)
                                        .then((fees) => setState(() => _feeEstimates = fees));
                                  },
                                ),
                                Visibility(
                                  visible: formFieldState.hasError &&
                                      formFieldState.errorText != sendAmountIsZero,
                                  child: Container(
                                    decoration: BoxDecoration(
                                        color: Colors.redAccent.shade100.withOpacity(0.1),
                                        border: Border.all(color: Colors.red),
                                        borderRadius: BorderRadius.circular(10)),
                                    padding: const EdgeInsets.all(10),
                                    child: Wrap(
                                      crossAxisAlignment: WrapCrossAlignment.center,
                                      children: [
                                        const Icon(Icons.info_outline,
                                            color: Colors.black87, size: 18),
                                        const SizedBox(width: 5),
                                        Text(
                                          formFieldState.errorText ?? "",
                                          textAlign: TextAlign.center,
                                          style:
                                              const TextStyle(color: Colors.black87, fontSize: 14),
                                        ),
                                      ],
                                    ),
                                  ),
                                )
                              ],
                            );
                          },
                        )),
                    const SizedBox(height: 8),
                    Center(
                      child: Padding(
                        padding: const EdgeInsets.only(right: 32.0),
                        child: Material(
                          color: _amount == null ? tenTenOnePurple : null,
                          borderRadius: BorderRadius.circular(16),
                          child: InkWell(
                            customBorder: RoundedRectangleBorder(
                              borderRadius: BorderRadius.circular(16),
                            ),
                            child: Padding(
                              padding: const EdgeInsets.all(10.0),
                              child: Text("Max",
                                  style: TextStyle(
                                    fontSize: 16,
                                    color: _amount == null ? Colors.white : tenTenOnePurple,
                                  )),
                            ),
                            onTap: () {
                              setState(() {
                                if (_amount != null) {
                                  _amount = null;
                                  _controller.text = "Max";
                                } else {
                                  _amount = Amount.zero();
                                  _controller.text = Amount.zero().formatted();
                                }
                              });

                              _walletService
                                  .calculateFeesForOnChain(widget.destination.address)
                                  .then((fees) => setState(() => _feeEstimates = fees));
                            },
                          ),
                        ),
                      ),
                    ),
                    Visibility(
                        visible: widget.destination.description != "",
                        child: Column(
                          children: [
                            Container(
                              padding:
                                  const EdgeInsets.only(top: 20, left: 20, right: 20, bottom: 20),
                              decoration: BoxDecoration(
                                  border: Border.all(color: Colors.grey.shade200),
                                  borderRadius: BorderRadius.circular(10),
                                  color: Colors.orange.shade200.withOpacity(0.1)),
                              child:
                                  Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                                const Text(
                                  "Memo:",
                                  style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold),
                                  textAlign: TextAlign.start,
                                ),
                                const SizedBox(height: 5),
                                Text(widget.destination.description,
                                    maxLines: 2,
                                    overflow: TextOverflow.ellipsis,
                                    softWrap: true,
                                    style: const TextStyle(fontSize: 16))
                              ]),
                            ),
                            const SizedBox(height: 15),
                          ],
                        )),
                    const SizedBox(height: 35),
                    Container(
                      padding: const EdgeInsets.all(20),
                      decoration: BoxDecoration(
                          border: Border.all(color: Colors.grey.shade200),
                          borderRadius: BorderRadius.circular(10),
                          color: Colors.orange.shade300.withOpacity(0.1)),
                      child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                        Row(mainAxisAlignment: MainAxisAlignment.spaceBetween, children: [
                          const Text("Available Balance",
                              overflow: TextOverflow.ellipsis, style: TextStyle(fontSize: 14)),
                          Text(balance.toString(),
                              overflow: TextOverflow.ellipsis,
                              style: const TextStyle(fontSize: 14)),
                        ])
                      ]),
                    ),
                    const SizedBox(height: 20),
                    const Text("Select Network Fee", style: TextStyle(fontSize: 16)),
                    const SizedBox(height: 10),
                    FeePicker(
                        initialSelection: _feeConfig,
                        feeEstimates: _feeEstimates,
                        customFee: _customFee,
                        onChange: (feeConfig) async {
                          setState(() => _feeConfig = feeConfig);
                          if (feeConfig is CustomFeeRate) {
                            await calculateCustomFee(feeConfig);
                          }
                        }),
                    const SizedBox(height: 20),
                    SizedBox(
                      width: MediaQuery.of(context).size.width * 0.9,
                      child: ElevatedButton(
                          onPressed: (_formKey.currentState?.validate() ?? false)
                              ? () => showConfirmPaymentModal(
                                    context,
                                    widget.destination,
                                    _amount,
                                    _feeConfig,
                                    currentFee()!,
                                  )
                              : null,
                          style: ButtonStyle(
                              padding:
                                  WidgetStateProperty.all<EdgeInsets>(const EdgeInsets.all(15)),
                              backgroundColor: WidgetStateProperty.resolveWith((states) {
                                if (states.contains(WidgetState.disabled)) {
                                  return tenTenOnePurple.shade100;
                                } else {
                                  return tenTenOnePurple;
                                }
                              }),
                              shape: WidgetStateProperty.resolveWith((states) {
                                if (states.contains(WidgetState.disabled)) {
                                  return RoundedRectangleBorder(
                                    borderRadius: BorderRadius.circular(30.0),
                                    side: BorderSide(color: tenTenOnePurple.shade100),
                                  );
                                } else {
                                  return RoundedRectangleBorder(
                                    borderRadius: BorderRadius.circular(30.0),
                                    side: const BorderSide(color: tenTenOnePurple),
                                  );
                                }
                              })),
                          child: const Text(
                            "Send",
                            style: TextStyle(fontSize: 18, color: Colors.white),
                          )),
                    )
                  ]),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

// We define a constant to avoid _displaying_ an error if the send amount is set
// to zero. It should be self-evident that sending zero sats is not supported,
// so it's enough to disable the `Send` button.
//
// This allows us to set the send amount to zero by default, without displaying
// an error.
const String sendAmountIsZero = "send-amount-is-zero";
