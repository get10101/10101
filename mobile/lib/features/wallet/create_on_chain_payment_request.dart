import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/amount_text_input_form_field.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/scrollable_safe_area.dart';
import 'package:get_10101/features/wallet/application/wallet_service.dart';
import 'package:get_10101/features/wallet/share_invoice_screen.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

import 'domain/share_invoice.dart';

class CreateOnChainPaymentRequestScreen extends StatefulWidget {
  static const route = "${WalletScreen.route}/$subRouteName";
  static const subRouteName = "create_on_chain_payment_request";

  const CreateOnChainPaymentRequestScreen({super.key});

  @override
  State<CreateOnChainPaymentRequestScreen> createState() =>
      _CreateOnChainPaymentRequestScreenState();
}

class _CreateOnChainPaymentRequestScreenState extends State<CreateOnChainPaymentRequestScreen> {
  Amount? amount;

  final _formKey = GlobalKey<FormState>();
  bool showValidationHint = false;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text("Receive funds on-chain")),
      body: Form(
        key: _formKey,
        child: GestureDetector(
          onTap: () {
            FocusScope.of(context).requestFocus(FocusNode());
          },
          behavior: HitTestBehavior.opaque,
          child: ScrollableSafeArea(
            child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
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
                        value: amount != null ? amount! : Amount(0),
                        hint: "e.g. ${formatSats(Amount(5000))}",
                        label: "Amount",
                        isLoading: false,
                        onChanged: (value) {
                          if (value.isEmpty) {
                            return;
                          }

                          setState(() => amount = Amount.parse(value));
                        },
                        validator: (value) {
                          if (value == null) {
                            return "Enter receive amount";
                          }

                          try {
                            int amount = int.parse(value);

                            if (amount <= 0) {
                              return "Min amount to receive is ${formatSats(Amount(1))}";
                            }
                          } on Exception {
                            return "Enter a number";
                          }

                          return null;
                        },
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
                          onPressed: (amount == null || amount == Amount(0))
                              ? null
                              : () {
                                  if (_formKey.currentState!.validate()) {
                                    showValidationHint = false;

                                    final WalletService walletService =
                                        context.read<WalletChangeNotifier>().service;
                                    String uri =
                                        'bitcoin:${walletService.getUnusedAddress()}?amount=${amount!.btc}';

                                    GoRouter.of(context).go(ShareInvoiceScreen.route,
                                        extra: ShareInvoice(
                                          rawInvoice: uri,
                                          invoiceAmount: amount!,
                                          isLightning: false,
                                        ));
                                  } else {
                                    setState(() {
                                      showValidationHint = true;
                                    });
                                  }
                                },
                          child: const Text("Next")),
                    ],
                  ),
                ),
              )
            ]),
          ),
        ),
      ),
    );
  }
}
