import 'dart:developer';

import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_text_input_form_field.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/wallet/domain/wallet_info.dart';
import 'package:get_10101/features/wallet/share_invoice_screen.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

import 'application/wallet_service.dart';

class CreateInvoiceScreen extends StatefulWidget {
  static const route = "${WalletScreen.route}/$subRouteName";
  static const subRouteName = "create_invoice";
  final WalletService walletService;

  const CreateInvoiceScreen({super.key, this.walletService = const WalletService()});

  @override
  State<CreateInvoiceScreen> createState() => _CreateInvoiceScreenState();
}

class _CreateInvoiceScreenState extends State<CreateInvoiceScreen> {
  Amount? amount;
  final TextEditingController _amountController = TextEditingController();

  @override
  Widget build(BuildContext context) {
    WalletInfo info = context.watch<WalletChangeNotifier>().walletInfo;
    log("Refresh receive screen: ${info.balances.onChain}");

    return Scaffold(
      appBar: AppBar(title: const Text("Receive funds")),
      body: SafeArea(
        child: Container(
          constraints: const BoxConstraints.expand(),
          child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
            const Center(
              child: Padding(
                padding: EdgeInsets.only(top: 25.0),
                child: Text(
                  "What is the amount?",
                  style: TextStyle(fontSize: 18.0, fontWeight: FontWeight.bold),
                ),
              ),
            ),
            Padding(
              padding: const EdgeInsets.all(32.0),
              child: AmountInputField(
                value: amount != null ? amount! : Amount(0),
                hint: "e.g. 2,000 sats",
                label: "Amount",
                controller: _amountController,
                onChanged: (value) {
                  if (value.isEmpty) {
                    return;
                  }

                  setState(() => amount = Amount.parse(value));
                },
              ),
            ),
            Expanded(
              child: Padding(
                padding: const EdgeInsets.all(32.0),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  mainAxisAlignment: MainAxisAlignment.end,
                  children: [
                    ElevatedButton(
                        onPressed: amount == null
                            ? null
                            : () {
                                widget.walletService.createInvoice(amount!).then((invoice) {
                                  if (invoice != null) {
                                    GoRouter.of(context)
                                        .go(ShareInvoiceScreen.route, extra: invoice);
                                  }
                                });
                              },
                        child: const Text("Next")),
                  ],
                ),
              ),
            )
          ]),
        ),
      ),
    );
  }
}
