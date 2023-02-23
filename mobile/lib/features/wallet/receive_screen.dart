import 'dart:developer';

import 'package:flutter/material.dart';
import 'package:get_10101/features/wallet/application/wallet_service.dart';
import 'package:get_10101/features/wallet/domain/wallet_info.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:provider/provider.dart';

class ReceiveScreen extends StatefulWidget {
  static const route = "${WalletScreen.route}/$subRouteName";
  static const subRouteName = "receive";
  final WalletService walletService;

  const ReceiveScreen({super.key, this.walletService = const WalletService()});

  @override
  State<ReceiveScreen> createState() => _ReceiveScreenState();
}

class _ReceiveScreenState extends State<ReceiveScreen> {
  String invoice = "";

  @override
  Widget build(BuildContext context) {
    WalletInfo info = context.watch<WalletChangeNotifier>().walletInfo;

    log("Refresh receive screen: ${info.balances.onChain}");

    return Scaffold(
      appBar: AppBar(title: const Text("Receive")),
      body: SafeArea(
          child: Column(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          const SizedBox(height: 50),
          SelectableText("Address: ${widget.walletService.getNewAddress()}"),
          Text("Balance: ${info.balances.lightning} / ${info.balances.onChain}"),
          ElevatedButton(
              onPressed: () {
                setState(() async {
                  String? invoice = await widget.walletService.createInvoice();
                  if (invoice != null) {
                    this.invoice = invoice;
                  }
                });
              },
              child: const Text("Create Invoice")),
          SelectableText("Invoice: $invoice"),
          ElevatedButton(
              onPressed: () async {
                await widget.walletService.openChannel();
              },
              child: const Text("Open Channel!"))
        ],
      )),
    );
  }
}
