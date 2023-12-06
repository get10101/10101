import 'dart:math';

import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/amount_text_input_form_field.dart';
import 'package:get_10101/common/application/channel_info_service.dart';
import 'package:get_10101/common/domain/channel.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/scrollable_safe_area.dart';
import 'package:get_10101/features/wallet/application/util.dart';
import 'package:get_10101/features/wallet/application/wallet_service.dart';
import 'package:get_10101/features/wallet/domain/destination.dart';
import 'package:get_10101/features/wallet/domain/wallet_type.dart';
import 'package:get_10101/features/wallet/send/confirm_payment_modal.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:provider/provider.dart';

class SendScreen extends StatefulWidget {
  static const route = "${WalletScreen.route}/$subRouteName";
  static const subRouteName = "send";

  final Destination destination;

  const SendScreen({super.key, required this.destination});

  @override
  State<SendScreen> createState() => _SendScreenState();
}

class _SendScreenState extends State<SendScreen> {
  final _formKey = GlobalKey<FormState>();
  bool _valid = false;

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
    final balance = getBalance();

    return Scaffold(
      appBar: AppBar(title: const Text("Send Funds")),
      body: Form(
        key: _formKey,
        child: ScrollableSafeArea(
          child: Container(
            padding: const EdgeInsets.all(20.0),
            child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
              const Text("Destination", style: TextStyle(fontWeight: FontWeight.bold)),
              const SizedBox(height: 2),
              InputDecorator(
                decoration: InputDecoration(
                  enabledBorder:
                      const OutlineInputBorder(borderSide: BorderSide(color: Colors.black12)),
                  labelStyle: const TextStyle(color: Colors.black87),
                  filled: true,
                  fillColor: Colors.grey[50],
                ),
                child: Text(truncateWithEllipsis(26, widget.destination.raw),
                    style: const TextStyle(fontSize: 15)),
              ),
              const SizedBox(height: 15),
              const Text("From", style: TextStyle(fontWeight: FontWeight.bold)),
              const SizedBox(height: 2),
              OutlinedButton(
                  onPressed: null,
                  style: OutlinedButton.styleFrom(
                    minimumSize: const Size(20, 60),
                    backgroundColor: Colors.white,
                    shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
                  ),
                  child: Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      Row(mainAxisAlignment: MainAxisAlignment.start, children: [
                        Icon(
                            widget.destination.getWalletType() == WalletType.lightning
                                ? Icons.bolt
                                : Icons.currency_bitcoin,
                            size: 30),
                        const SizedBox(width: 5),
                        Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              widget.destination.getWalletType() == WalletType.lightning
                                  ? "Lightning"
                                  : "On-chain",
                              style: const TextStyle(color: Colors.black87, fontSize: 16),
                            ),
                            Text(formatSats(balance[widget.destination.getWalletType()]!.$1))
                          ],
                        ),
                      ]),
                      const Icon(Icons.arrow_drop_down_sharp, size: 30)
                    ],
                  )),
              const SizedBox(height: 20),
              Visibility(
                visible: widget.destination.getWalletType() == WalletType.onChain,
                replacement: const Text(
                  "Amount in sats",
                  style: TextStyle(fontWeight: FontWeight.bold),
                ),
                child: const Text(
                  "Amount in sats (0 to drain the wallet)",
                  style: TextStyle(fontWeight: FontWeight.bold),
                ),
              ),
              const SizedBox(height: 2),
              AmountInputField(
                controller: _controller,
                label: "",
                value: _amount,
                enabled: widget.destination.amount.sats == 0,
                onChanged: (value) {
                  setState(() {
                    _amount = Amount.parseAmount(value);
                    _valid = _formKey.currentState?.validate() ?? false;
                  });
                },
                validator: (value) {
                  if (value == null || value.isEmpty) {
                    return "Amount is mandatory";
                  }

                  final amount = Amount.parseAmount(value);

                  if (amount.sats <= 0 &&
                      widget.destination.getWalletType() == WalletType.lightning) {
                    return "Amount cannot be 0";
                  }

                  if (amount.sats < 0) {
                    return "Amount cannot be negative";
                  }

                  final bal = balance[widget.destination.getWalletType()]!.$1;
                  if (amount.sats > bal.sats) {
                    return "Not enough funds.";
                  }

                  final usebal = balance[widget.destination.getWalletType()]!.$2;

                  if (amount.sats > usebal.sats) {
                    return "Not enough funds. ${formatSats(bal.sub(usebal))} have to remain.";
                  }

                  return null;
                },
              ),
              const SizedBox(height: 20),
              const Text("Note", style: TextStyle(fontWeight: FontWeight.bold)),
              const SizedBox(height: 2),
              InputDecorator(
                decoration: InputDecoration(
                  enabledBorder:
                      const OutlineInputBorder(borderSide: BorderSide(color: Colors.black12)),
                  labelStyle: const TextStyle(color: Colors.black87),
                  filled: true,
                  fillColor: Colors.grey[50],
                ),
                child: Text(widget.destination.description, style: const TextStyle(fontSize: 15)),
              ),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  mainAxisAlignment: MainAxisAlignment.end,
                  children: [
                    ElevatedButton(
                        onPressed: !_valid
                            ? null
                            : () => showConfirmPaymentModal(context, widget.destination, _amount),
                        child: const Text("Next")),
                  ],
                ),
              )
            ]),
          ),
        ),
      ),
    );
  }

  Map<WalletType, (Amount, Amount)> getBalance() {
    final walletInfo = context.read<WalletChangeNotifier>().walletInfo;
    final ChannelInfoService channelInfoService = context.read<ChannelInfoService>();
    Amount initialReserve = channelInfoService.getInitialReserve();
    int channelReserve = channelInfo?.reserve.sats ?? initialReserve.sats;
    int balance = walletInfo.balances.lightning.sats;
    return {
      WalletType.lightning: (Amount(balance), Amount(max(balance - channelReserve, 0))),
      WalletType.onChain: (walletInfo.balances.onChain, walletInfo.balances.onChain)
    };
  }
}
