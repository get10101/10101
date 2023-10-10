import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/amount_text_input_form_field.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/wallet/domain/share_invoice.dart';
import 'package:get_10101/features/wallet/share_invoice_screen.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

class CreateInvoiceScreen extends StatefulWidget {
  static const route = "${WalletScreen.route}/$subRouteName";
  static const subRouteName = "create_invoice";

  const CreateInvoiceScreen({super.key});

  @override
  State<CreateInvoiceScreen> createState() => _CreateInvoiceScreenState();
}

class _CreateInvoiceScreenState extends State<CreateInvoiceScreen> {
  Amount? amount;

  @override
  Widget build(BuildContext context) {
    final WalletChangeNotifier walletChangeNotifier = context.watch<WalletChangeNotifier>();

    return Scaffold(
      appBar: AppBar(title: const Text("Receive funds on Lightning")),
      body: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        const Center(
            child: Padding(
                padding: EdgeInsets.only(top: 25.0),
                child: Text(
                  "What is the amount?",
                  style: TextStyle(fontSize: 18.0, fontWeight: FontWeight.bold),
                ))),
        Padding(
          padding: const EdgeInsets.all(32.0),
          child: Row(
            children: [
              Expanded(
                child: AmountInputField(
                  value: amount ?? Amount(0),
                  hint: "e.g. ${formatSats(Amount(50000))}",
                  label: "Amount",
                  onChanged: (value) {
                    if (value.isEmpty) {
                      return;
                    }

                    setState(() => amount = Amount.parseAmount(value));
                  },
                  isLoading: false,
                ),
              ),
            ],
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
                    onPressed: () {
                      walletChangeNotifier.service.createInvoice(amount!).then((invoice) {
                        if (invoice != null) {
                          GoRouter.of(context).go(ShareInvoiceScreen.route,
                              extra: ShareInvoice(
                                  rawInvoice: invoice, isLightning: true, invoiceAmount: amount!));
                        }
                      });
                    },
                    child: const Text("Next")),
              ],
            ),
          ),
        )
      ]),
    );
  }
}
