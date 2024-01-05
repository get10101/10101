import 'dart:math';

import 'package:bitcoin_icons/bitcoin_icons.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/common/custom_app_bar.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/application/channel_info_service.dart';
import 'package:get_10101/common/application/switch.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/domain/channel.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/scrollable_safe_area.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/features/trade/trade_value_change_notifier.dart';
import 'package:get_10101/features/wallet/application/util.dart';
import 'package:get_10101/features/wallet/application/wallet_service.dart';
import 'package:get_10101/features/wallet/domain/destination.dart';
import 'package:get_10101/features/wallet/domain/wallet_type.dart';
import 'package:get_10101/features/wallet/send/confirm_payment_modal.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:intl/intl.dart';
import 'package:provider/provider.dart';

class SendLightningScreen extends StatefulWidget {
  static const route = "${WalletScreen.route}/$subRouteName";
  static const subRouteName = "send-lightning";

  final LightningInvoice destination;

  const SendLightningScreen({super.key, required this.destination});

  @override
  State<SendLightningScreen> createState() => _SendLightningScreenState();
}

class _SendLightningScreenState extends State<SendLightningScreen> {
  final _satsFormKey = GlobalKey<FormState>();
  final _usdpFormKey = GlobalKey<FormState>();

  bool _payWithUsdp = false;

  ChannelInfo? channelInfo;

  Amount _satsAmount = Amount.zero();
  Amount _usdpAmount = Amount.zero();

  final TextEditingController _satsController = TextEditingController();
  final TextEditingController _usdpController = TextEditingController();

  @override
  void initState() {
    super.initState();
    final ChannelInfoService channelInfoService = context.read<ChannelInfoService>();
    final WalletService walletService = context.read<WalletChangeNotifier>().service;
    final tradeValueChangeNotifier = context.read<TradeValuesChangeNotifier>();
    init(channelInfoService, walletService, tradeValueChangeNotifier);
  }

  @override
  void dispose() {
    super.dispose();
    _satsController.dispose();
    _usdpController.dispose();
  }

  Future<void> init(ChannelInfoService channelInfoService, WalletService walletService,
      TradeValuesChangeNotifier tradeValuesChangeNotifier) async {
    channelInfo = await channelInfoService.getChannelInfo();
    setState(() {
      _satsAmount = widget.destination.amount;
      _satsController.text = _satsAmount.formatted();

      final tradeValues = tradeValuesChangeNotifier.fromDirection(Direction.long);
      tradeValues.updateLeverage(Leverage(1));
      tradeValues.updateMargin(_satsAmount);

      _usdpAmount = tradeValues.quantity ?? Amount.zero();
      _usdpController.text = _usdpAmount.formatted();
    });
  }

  @override
  Widget build(BuildContext context) {
    final positionChangeNotifier = context.read<PositionChangeNotifier>();
    final tradeValuesChangeNotifier = context.watch<TradeValuesChangeNotifier>();

    final formatter = NumberFormat("#,###,##0.00", "en");

    final usdpBalance = positionChangeNotifier.getStableUSDAmountInFiat();
    final offChainBalance = getOffChainBalance();

    return GestureDetector(
      onTap: () => FocusManager.instance.primaryFocus?.unfocus(),
      child: Scaffold(
        resizeToAvoidBottomInset: true,
        body: ScrollableSafeArea(
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
                    color: tenTenOnePurple.shade200.withOpacity(0.1)),
                child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                  const Text(
                    "Pay to:",
                    style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold),
                    textAlign: TextAlign.start,
                  ),
                  const SizedBox(height: 2),
                  Row(mainAxisAlignment: MainAxisAlignment.spaceBetween, children: [
                    Text(truncateWithEllipsis(18, widget.destination.raw),
                        overflow: TextOverflow.ellipsis, style: const TextStyle(fontSize: 16)),
                    Container(
                      padding: const EdgeInsets.only(left: 10, right: 10, top: 5, bottom: 5),
                      decoration: BoxDecoration(
                        color: tenTenOnePurple,
                        border: Border.all(color: Colors.grey.shade200),
                        borderRadius: BorderRadius.circular(20),
                      ),
                      child: const Row(
                        mainAxisAlignment: MainAxisAlignment.spaceBetween,
                        children: [
                          Icon(BitcoinIcons.lightning, size: 14, color: Colors.white),
                          Text("Lightning", style: TextStyle(fontSize: 14, color: Colors.white))
                        ],
                      ),
                    )
                  ])
                ]),
              ),
              const SizedBox(height: 25),
              Text(
                isZeroValueInvoice ? "Enter amount" : "Amount",
                textAlign: TextAlign.center,
                style: const TextStyle(fontSize: 14, color: Colors.grey),
              ),
              const SizedBox(height: 10),
              Container(
                  margin: const EdgeInsets.only(left: 40, right: 40),
                  child: Visibility(
                      maintainState: true,
                      visible: !_payWithUsdp,
                      child: buildSatsForm(
                          tradeValuesChangeNotifier, offChainBalance.$1, offChainBalance.$2))),
              Container(
                margin: const EdgeInsets.only(left: 40, right: 40),
                child: Visibility(
                  maintainState: true,
                  visible: _payWithUsdp,
                  child: buildUsdpForm(tradeValuesChangeNotifier, usdpBalance),
                ),
              ),
              const SizedBox(height: 25),
              Visibility(
                  visible: widget.destination.description != "",
                  child: Column(
                    children: [
                      Container(
                        padding: const EdgeInsets.only(top: 20, left: 20, right: 20, bottom: 20),
                        decoration: BoxDecoration(
                            border: Border.all(color: Colors.grey.shade200),
                            borderRadius: BorderRadius.circular(10),
                            color: tenTenOnePurple.shade200.withOpacity(0.1)),
                        child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
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
              const Text(
                "Pay from:",
                style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold),
                textAlign: TextAlign.start,
              ),
              const SizedBox(height: 5),
              Container(
                decoration: BoxDecoration(
                    border: Border.all(color: Colors.grey.shade200),
                    borderRadius: BorderRadius.circular(10),
                    color: tenTenOnePurple.shade200.withOpacity(0.1)),
                child: Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                    GestureDetector(
                      onTap: () => setState(() => _payWithUsdp = false),
                      child: Container(
                        padding: const EdgeInsets.all(20),
                        child: Opacity(
                            opacity: _payWithUsdp ? 0.5 : 1.0,
                            child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                              const Row(
                                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                                  children: [
                                    Icon(BitcoinIcons.lightning, size: 18),
                                    Text("Lightning", style: TextStyle(fontSize: 18))
                                  ]),
                              const SizedBox(height: 5),
                              Text(offChainBalance.$2.toString(), textAlign: TextAlign.start),
                            ])),
                      ),
                    ),
                    TenTenOneSwitch(
                        value: _payWithUsdp,
                        isDisabled: usdpBalance <= 0,
                        showDisabled: !_payWithUsdp,
                        onChanged: (value) => setState(() => _payWithUsdp = value)),
                    GestureDetector(
                      onTap: () => setState(() => _payWithUsdp = true),
                      child: Container(
                        padding: const EdgeInsets.all(20),
                        child: Opacity(
                          opacity: _payWithUsdp ? 1.0 : 0.5,
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.end,
                            children: [
                              const Text("USD-P", style: TextStyle(fontSize: 18)),
                              const SizedBox(height: 5),
                              Text(formatter.format(usdpBalance), textAlign: TextAlign.end),
                            ],
                          ),
                        ),
                      ),
                    )
                  ],
                ),
              ),
              const SizedBox(height: 2),
              const Spacer(),
              const SizedBox(height: 10),
              SizedBox(
                width: MediaQuery.of(context).size.width * 0.9,
                child: ElevatedButton(
                    onPressed: (_payWithUsdp
                            ? (_usdpFormKey.currentState?.validate() ?? false)
                            : (_satsFormKey.currentState?.validate() ?? false))
                        ? () => showConfirmPaymentModal(
                            context, widget.destination, _payWithUsdp, _satsAmount, _usdpAmount)
                        : null,
                    style: ButtonStyle(
                        padding: MaterialStateProperty.all<EdgeInsets>(const EdgeInsets.all(15)),
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
                      "Pay",
                      style: TextStyle(fontSize: 18, color: Colors.white),
                    )),
              ),
            ]),
          ),
        ),
      ),
    );
  }

  Form buildUsdpForm(TradeValuesChangeNotifier tradeValuesChangeNotifier, double usdpBalance) {
    return Form(
      key: _usdpFormKey,
      child: FormField<String>(
        validator: (val) {
          final amount = _usdpAmount;

          if (amount.sats <= 0) {
            return "Amount cannot be 0";
          }

          if (amount.sats < 0) {
            return "Amount cannot be negative";
          }

          if (amount.sats > usdpBalance) {
            return "Not enough funds.";
          }

          return null;
        },
        builder: (FormFieldState<String> formFieldState) {
          return Column(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              TextField(
                keyboardType: TextInputType.number,
                textAlign: TextAlign.center,
                controller: _usdpController,
                decoration: const InputDecoration(
                    hintText: "0.00",
                    hintStyle: TextStyle(fontSize: 40),
                    enabledBorder: InputBorder.none,
                    border: InputBorder.none,
                    errorBorder: InputBorder.none,
                    suffix: Text(
                      "\$",
                      style: TextStyle(fontSize: 16),
                    )),
                style: const TextStyle(fontSize: 40),
                textAlignVertical: TextAlignVertical.center,
                enabled: isZeroValueInvoice,
                onChanged: (value) {
                  setState(() {
                    _usdpAmount = Amount.parseAmount(value);
                    final tradeValues = tradeValuesChangeNotifier.fromDirection(Direction.long);
                    tradeValues.updateQuantity(_usdpAmount);
                    _usdpController.text = _usdpAmount.formatted();

                    _satsAmount = tradeValues.margin ?? Amount.zero();
                    _satsController.text = _satsAmount.formatted();
                    _satsController.selection =
                        TextSelection.collapsed(offset: _satsController.text.length);
                  });
                },
              ),
              Visibility(
                visible: formFieldState.hasError,
                replacement: Container(margin: const EdgeInsets.only(top: 30, bottom: 10)),
                child: Container(
                  decoration: BoxDecoration(
                      color: Colors.redAccent.shade100.withOpacity(0.1),
                      border: Border.all(color: Colors.red),
                      borderRadius: BorderRadius.circular(10)),
                  padding: const EdgeInsets.all(10),
                  child: Wrap(
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: [
                      const Icon(Icons.info_outline, color: Colors.black87, size: 18),
                      const SizedBox(width: 5),
                      Text(
                        formFieldState.errorText ?? "",
                        textAlign: TextAlign.center,
                        style: const TextStyle(color: Colors.black87, fontSize: 14),
                      ),
                    ],
                  ),
                ),
              )
            ],
          );
        },
      ),
    );
  }

  Form buildSatsForm(
      TradeValuesChangeNotifier tradeValuesChangeNotifier, Amount balance, Amount useableBalance) {
    return Form(
      key: _satsFormKey,
      child: FormField(
        validator: (val) {
          final amount = _satsAmount;

          if (amount.sats <= 0 && widget.destination.getWalletType() == WalletType.lightning) {
            return "Amount cannot be 0";
          }

          if (amount.sats < 0) {
            return "Amount cannot be negative";
          }

          if (amount.sats > balance.sats) {
            return "Not enough funds.";
          }

          if (amount.sats > useableBalance.sats) {
            return "Not enough funds. ${formatSats(balance.sub(useableBalance))} have to remain.";
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
                controller: _satsController,
                onChanged: (value) {
                  setState(() {
                    _satsAmount = Amount.parseAmount(value);
                    final tradeValues = tradeValuesChangeNotifier.fromDirection(Direction.long);
                    tradeValues.updateMargin(_satsAmount);
                    _satsController.text = _satsAmount.formatted();

                    _usdpAmount = tradeValues.quantity ?? Amount.zero();
                    _usdpController.text = _usdpAmount.formatted();
                    _usdpController.selection =
                        TextSelection.collapsed(offset: _usdpController.text.length);
                  });
                },
              ),
              Visibility(
                visible: formFieldState.hasError,
                replacement: Container(margin: const EdgeInsets.only(top: 30, bottom: 10)),
                child: Container(
                  decoration: BoxDecoration(
                      color: Colors.redAccent.shade100.withOpacity(0.1),
                      border: Border.all(color: Colors.red),
                      borderRadius: BorderRadius.circular(10)),
                  padding: const EdgeInsets.all(10),
                  child: Wrap(
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: [
                      const Icon(Icons.info_outline, color: Colors.black87, size: 18),
                      const SizedBox(width: 5),
                      Text(
                        formFieldState.errorText ?? "",
                        textAlign: TextAlign.center,
                        style: const TextStyle(color: Colors.black87, fontSize: 14),
                      ),
                    ],
                  ),
                ),
              )
            ],
          );
        },
      ),
    );
  }

  (Amount, Amount) getOffChainBalance() {
    final walletInfo = context.read<WalletChangeNotifier>().walletInfo;
    final ChannelInfoService channelInfoService = context.read<ChannelInfoService>();
    Amount initialReserve = channelInfoService.getInitialReserve();
    int channelReserve = channelInfo?.reserve.sats ?? initialReserve.sats;
    int balance = walletInfo.balances.offChain.sats;

    return (Amount(balance), Amount(max(balance - channelReserve, 0)));
  }

  bool get isZeroValueInvoice => widget.destination.amount.sats == 0;
}
