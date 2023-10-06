import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/application/channel_info_service.dart';
import 'package:get_10101/common/domain/channel.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/scrollable_safe_area.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/wallet/application/wallet_service.dart';
import 'package:get_10101/features/wallet/domain/lightning_invoice.dart';
import 'package:get_10101/features/wallet/send_payment_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:get_10101/logger.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

class SendPaymentScreen extends StatefulWidget {
  static const route = "${WalletScreen.route}/$subRouteName";
  static const subRouteName = "send";

  const SendPaymentScreen({super.key});

  @override
  State<SendPaymentScreen> createState() => _SendPaymentScreenState();
}

class _SendPaymentScreenState extends State<SendPaymentScreen> {
  final TextEditingController _textEditingController = TextEditingController();
  final _formKey = GlobalKey<FormState>();
  LightningInvoice? _lightningInvoice;
  bool isDecoding = false;
  bool decodingFailed = false;
  ChannelInfo? channelInfo;

  @override
  void initState() {
    final ChannelInfoService channelInfoService = context.read<ChannelInfoService>();
    initChannelValues(channelInfoService);
    final WalletService walletService = context.read<WalletChangeNotifier>().service;
    invoiceFromClipboard(walletService);
    super.initState();
  }

  Future<void> initChannelValues(ChannelInfoService channelInfoService) async {
    channelInfo = await channelInfoService.getChannelInfo();
  }

  invoiceFromClipboard(WalletService walletService) async {
    ClipboardData? clipboard = await Clipboard.getData(Clipboard.kTextPlain);

    if (clipboard == null || clipboard.text == null) {
      return;
    }

    _lightningInvoice = await walletService.decodeInvoice(clipboard.text!);

    if (_lightningInvoice != null) {
      _textEditingController.text = clipboard.text!;
    }
  }

  @override
  void dispose() {
    _textEditingController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final ChannelInfoService channelInfoService = context.read<ChannelInfoService>();
    final WalletService walletService = context.read<WalletChangeNotifier>().service;
    Amount initialReserve = channelInfoService.getInitialReserve();
    int channelReserve = channelInfo?.reserve.sats ?? initialReserve.sats;
    int balance = context.watch<WalletChangeNotifier>().walletInfo.balances.lightning.sats;
    int maxSendAmount = max(balance - channelReserve, 0);

    return Scaffold(
      appBar: AppBar(title: const Text("Send")),
      body: Form(
        key: _formKey,
        child: GestureDetector(
          onTap: () {
            FocusScope.of(context).requestFocus(FocusNode());
          },
          behavior: HitTestBehavior.opaque,
          child: ScrollableSafeArea(
            child: Container(
              constraints: const BoxConstraints.expand(),
              child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
                const Center(
                    child: Padding(
                        padding: EdgeInsets.only(top: 25.0),
                        child: Text(
                          "Invoice:",
                          style: TextStyle(fontSize: 18.0, fontWeight: FontWeight.bold),
                        ))),
                Padding(
                  padding: const EdgeInsets.all(32.0),
                  child: TextFormField(
                    decoration: const InputDecoration(
                      border: OutlineInputBorder(),
                      hintText: "Paste your invoice here",
                      labelText: "Invoice",
                    ),
                    controller: _textEditingController,
                    onChanged: (value) async {
                      logger.d(value);

                      setState(() {
                        isDecoding = true;
                      });

                      LightningInvoice? decoded =
                          await walletService.decodeInvoice(value.toString());

                      setState(() {
                        isDecoding = false;
                      });

                      if (decoded == null) {
                        setState(() {
                          decodingFailed = true;
                        });
                        return;
                      }

                      setState(() {
                        decodingFailed = false;
                        _lightningInvoice = decoded;
                      });
                    },
                    validator: (value) {
                      if (_lightningInvoice == null) {
                        return "Failed to decode invoice";
                      }

                      return null;
                    },
                  ),
                ),
                if (isDecoding)
                  const Padding(
                    padding: EdgeInsets.symmetric(horizontal: 32),
                    child: Text("Decoding invoice, please wait..."),
                  )
                else if (decodingFailed)
                  const Padding(
                    padding: EdgeInsets.symmetric(horizontal: 32),
                    child: Text(
                      "Decoding failed, invalid invoice!",
                      style: TextStyle(color: Colors.red),
                    ),
                  )
                else if (_lightningInvoice != null)
                  Center(
                    child: Padding(
                      padding: const EdgeInsets.all(32.0),
                      child: Column(
                        children: [
                          const Text("Invoice Data:"),
                          const SizedBox(
                            height: 10,
                          ),
                          ValueDataRow(
                            type: ValueType.amount,
                            value: _lightningInvoice!.amountSats,
                            label: "Amount",
                          ),
                          const SizedBox(
                            height: 5,
                          ),
                          ValueDataRow(
                            type: ValueType.text,
                            value: _lightningInvoice!.payee,
                            label: "Recipient",
                          ),
                          const SizedBox(
                            height: 5,
                          ),
                          ValueDataRow(
                            type: ValueType.date,
                            value: _lightningInvoice!.expiry,
                            label: "Expiry",
                          ),
                          if (_lightningInvoice!.amountSats == Amount.zero())
                            const Text("Invoices without amount are not supported yet",
                                style: TextStyle(color: Colors.red))
                        ],
                      ),
                    ),
                  ),
                Center(
                    child: Padding(
                  padding: const EdgeInsets.only(bottom: 10.0, left: 32.0, right: 32.0),
                  child: Text(
                    "During the beta a minimum of $channelReserve sats have to remain in the wallet."
                    "\nYour wallet balance is $balance sats so you can send up to $maxSendAmount sats.",
                    style: const TextStyle(color: Colors.grey),
                  ),
                )),
                Expanded(
                  child: Padding(
                    padding: const EdgeInsets.all(32.0),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      mainAxisAlignment: MainAxisAlignment.end,
                      children: [
                        ElevatedButton(
                            onPressed: !(_formKey.currentState?.validate() ?? false)
                                ? null
                                : () async {
                                    context.read<SendPaymentChangeNotifier>().sendPayment(
                                        _textEditingController.text, _lightningInvoice!);
                                    GoRouter.of(context).go(WalletScreen.route);
                                  },
                            child: const Text("Send Payment")),
                      ],
                    ),
                  ),
                )
              ]),
            ),
          ),
        ),
      ),
    );
  }
}
