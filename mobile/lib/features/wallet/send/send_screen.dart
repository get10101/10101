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
import 'package:get_10101/features/wallet/send/enter_destination_modal.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:provider/provider.dart';

class SendScreen extends StatefulWidget {
  static const route = "${WalletScreen.route}/$subRouteName";
  static const subRouteName = "send";

  const SendScreen({super.key});

  @override
  State<SendScreen> createState() => _SendScreenState();
}

class _SendScreenState extends State<SendScreen> {
  final _formKey = GlobalKey<FormState>();
  bool _valid = false;
  bool _invalidDestination = false;

  ChannelInfo? channelInfo;

  Destination? _destination;
  Amount? _amount;

  final TextEditingController _controller = TextEditingController();

  @override
  void initState() {
    super.initState();
    final ChannelInfoService channelInfoService = context.read<ChannelInfoService>();
    initChannelValues(channelInfoService);
  }

  @override
  void dispose() {
    super.dispose();
    _controller.dispose();
  }

  Future<void> initChannelValues(ChannelInfoService channelInfoService) async {
    channelInfo = await channelInfoService.getChannelInfo();
  }

  @override
  Widget build(BuildContext context) {
    final WalletService walletService = context.watch<WalletChangeNotifier>().service;

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
              OutlinedButton(
                  onPressed: () => showEnterDestinationModal(context, (encodedDestination) {
                        walletService.decodeDestination(encodedDestination).then((destination) {
                          if (destination == null) {
                            logger.w("Invalid destination!");
                            setState(() => _invalidDestination = true);
                            return;
                          }

                          setState(() {
                            _destination = destination;
                            _amount = destination.amount;
                            _controller.text = _amount!.formatted();

                            _invalidDestination = false;
                            _valid = _formKey.currentState?.validate() ?? false;
                          });
                        });
                      }),
                  style: OutlinedButton.styleFrom(
                    side:
                        BorderSide(color: _invalidDestination ? Colors.red[900]! : Colors.black87),
                    minimumSize: const Size(20, 60),
                    backgroundColor: Colors.white,
                    shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
                  ),
                  child: Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      Text(
                          _destination?.raw != null
                              ? truncateWithEllipsis(26, _destination!.raw)
                              : "Set destination",
                          style: const TextStyle(color: Colors.black87, fontSize: 16)),
                      const Icon(Icons.edit, size: 20)
                    ],
                  )),
              Visibility(
                  visible: _invalidDestination,
                  child: Padding(
                      padding: const EdgeInsets.only(left: 10, top: 10, bottom: 10),
                      child: Text("Invalid destination",
                          style: TextStyle(color: Colors.red[900], fontSize: 12)))),
              const SizedBox(height: 15),
              const Text("From", style: TextStyle(fontWeight: FontWeight.bold)),
              const SizedBox(height: 2),
              OutlinedButton(
                  onPressed: null,
                  style: OutlinedButton.styleFrom(
                    minimumSize: const Size(20, 60),
                    backgroundColor: _destination != null ? Colors.white : Colors.white24,
                    shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
                  ),
                  child: Visibility(
                    visible: _destination != null,
                    child: Row(
                      mainAxisAlignment: MainAxisAlignment.spaceBetween,
                      children: [
                        _destination != null
                            ? Row(mainAxisAlignment: MainAxisAlignment.start, children: [
                                Icon(
                                    _destination!.getWalletType() == WalletType.lightning
                                        ? Icons.bolt
                                        : Icons.currency_bitcoin,
                                    size: 30),
                                const SizedBox(width: 5),
                                Column(
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: [
                                    Text(
                                      _destination!.getWalletType() == WalletType.lightning
                                          ? "Lightning"
                                          : "On-chain",
                                      style: const TextStyle(color: Colors.black87, fontSize: 16),
                                    ),
                                    Text(formatSats(balance[_destination!.getWalletType()]!.$1))
                                  ],
                                ),
                              ])
                            : Container(),
                        const Icon(Icons.arrow_drop_down_sharp, size: 30)
                      ],
                    ),
                  )),
              const SizedBox(height: 20),
              const Text("Amount in sats", style: TextStyle(fontWeight: FontWeight.bold)),
              const SizedBox(height: 2),
              AmountInputField(
                controller: _controller,
                label: "",
                value: _amount ?? Amount.zero(),
                enabled: _destination != null && _destination!.amount.sats == 0,
                onChanged: (value) {
                  setState(() {
                    _amount = Amount.parseAmount(value);
                    _valid = _formKey.currentState?.validate() ?? false;
                    logger.i("valid --> $_valid");
                  });
                },
                validator: (value) {
                  if (value == null || value.isEmpty) {
                    return "Amount is mandatory";
                  }

                  final amount = Amount.parseAmount(value);

                  if (amount.sats <= 0) {
                    return "Amount is mandatory";
                  }

                  if (_destination == null) {
                    return "Missing destination";
                  }

                  final bal = balance[_destination!.getWalletType()]!.$1;
                  if (amount.sats > bal.sats) {
                    return "Not enough funds.";
                  }

                  final usebal = balance[_destination!.getWalletType()]!.$2;

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
                child: Text(_destination != null ? _destination!.description : "",
                    style: const TextStyle(fontSize: 15)),
              ),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  mainAxisAlignment: MainAxisAlignment.end,
                  children: [
                    ElevatedButton(
                        onPressed: !_valid
                            ? null
                            : () => showConfirmPaymentModal(context, _destination!, _amount),
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
