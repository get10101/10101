import 'package:flutter/material.dart';
import 'package:get_10101/common/custom_app_bar.dart';
import 'package:get_10101/common/application/channel_info_service.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/domain/channel.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/scrollable_safe_area.dart';
import 'package:get_10101/features/wallet/application/util.dart';
import 'package:get_10101/features/wallet/application/wallet_service.dart';
import 'package:get_10101/features/wallet/domain/destination.dart';
import 'package:get_10101/features/wallet/send/confirm_payment_modal.dart';
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

  ChannelInfo? channelInfo;

  Amount _amount = Amount.zero();

  final TextEditingController _controller = TextEditingController();

  @override
  void initState() {
    super.initState();
    final ChannelInfoService channelInfoService = context.read<ChannelInfoService>();
    final WalletService walletService = context.read<WalletChangeNotifier>().service;
    init(channelInfoService, walletService);
  }

  @override
  void dispose() {
    super.dispose();
    _controller.dispose();
  }

  Future<void> init(ChannelInfoService channelInfoService, WalletService walletService) async {
    channelInfo = await channelInfoService.getChannelInfo();
    setState(() {
      _amount = widget.destination.amount;
      _controller.text = _amount.formatted();
    });
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
                      "Enter amount (0 to send the maximum)",
                      textAlign: TextAlign.center,
                      style: TextStyle(fontSize: 14, color: Colors.grey),
                    ),
                    const SizedBox(height: 10),
                    Container(
                        margin: const EdgeInsets.only(left: 40, right: 40),
                        child: FormField(
                          validator: (val) {
                            final amount = _amount;

                            if (amount.sats < 0) {
                              return "Amount cannot be negative";
                            }

                            if (amount.sats > balance.sats) {
                              return "Not enough funds.";
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
                                      hintText: "0.00",
                                      hintStyle: TextStyle(fontSize: 40),
                                      enabledBorder: InputBorder.none,
                                      border: InputBorder.none,
                                      errorBorder: InputBorder.none,
                                      suffix: Text(
                                        "sats",
                                        style: TextStyle(fontSize: 16),
                                      )),
                                  style: const TextStyle(fontSize: 40),
                                  textAlignVertical: TextAlignVertical.center,
                                  enabled: widget.destination.amount.sats == 0,
                                  controller: _controller,
                                  onChanged: (value) {
                                    setState(() {
                                      _amount = Amount.parseAmount(value);
                                      _controller.text = _amount.formatted();
                                    });
                                  },
                                ),
                                Visibility(
                                  visible: formFieldState.hasError,
                                  replacement:
                                      Container(margin: const EdgeInsets.only(top: 30, bottom: 10)),
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
                    const Spacer(),
                    SizedBox(
                      width: MediaQuery.of(context).size.width * 0.9,
                      child: ElevatedButton(
                          onPressed: (_formKey.currentState?.validate() ?? false)
                              ? () => showConfirmPaymentModal(
                                  context, widget.destination, false, _amount, _amount)
                              : null,
                          style: ButtonStyle(
                              padding:
                                  MaterialStateProperty.all<EdgeInsets>(const EdgeInsets.all(15)),
                              backgroundColor: MaterialStateProperty.resolveWith((states) {
                                if (states.contains(MaterialState.disabled)) {
                                  return tenTenOnePurple.shade100;
                                } else {
                                  return tenTenOnePurple;
                                }
                              }),
                              shape: MaterialStateProperty.resolveWith((states) {
                                if (states.contains(MaterialState.disabled)) {
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
