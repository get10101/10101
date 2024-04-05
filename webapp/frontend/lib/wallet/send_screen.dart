import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/model.dart';
import 'package:get_10101/common/amount_text_input_form_field.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/common/text_input_field.dart';
import 'package:get_10101/change_notifier/wallet_change_notifier.dart';
import 'package:provider/provider.dart';

class SendScreen extends StatefulWidget {
  const SendScreen({super.key});

  @override
  State<SendScreen> createState() => _SendScreenState();
}

class _SendScreenState extends State<SendScreen> {
  final GlobalKey<FormState> _formKey = GlobalKey<FormState>();

  final TextEditingController _addressController = TextEditingController();
  final TextEditingController _amountController = TextEditingController();
  final TextEditingController _feeController = TextEditingController();

  String? address;
  Amount? amount;
  Amount? fee;

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        Container(
            width: 450,
            padding: const EdgeInsets.all(25),
            child: Form(
              key: _formKey,
              child: Column(children: [
                TextInputField(
                  decoration: InputDecoration(
                      border: const OutlineInputBorder(),
                      filled: true,
                      fillColor: Colors.white,
                      labelStyle: const TextStyle(color: Colors.black87),
                      labelText: "Send to address",
                      errorStyle: TextStyle(
                        color: Colors.red[900],
                      ),
                      suffixIcon: GestureDetector(
                        child: const Icon(Icons.paste_rounded, size: 20),
                        onTap: () async {
                          final data = await Clipboard.getData("text/plain");
                          if (data?.text != null) {
                            setState(() {
                              address = data!.text;
                              _addressController.text = address!;
                            });
                          }
                        },
                      )),
                  controller: _addressController,
                  value: '',
                  label: "Send to address",
                  onChanged: (value) {
                    setState(() => address = value);
                  },
                  validator: (value) {
                    if (value == null || value.isEmpty) {
                      return "Please provide an address";
                    }
                    return null;
                  },
                ),
                const SizedBox(height: 20),
                AmountInputField(
                  initialValue: amount != null ? amount! : Amount.zero(),
                  label: "Amount in sats",
                  controller: _amountController,
                  validator: (value) {
                    return null;
                  },
                  onChanged: (value) {
                    if (value.isEmpty) {
                      amount = null;
                    }
                    setState(() => amount = Amount.parseAmount(value));
                  },
                ),
                const SizedBox(height: 20),
                AmountInputField(
                  initialValue: fee != null ? fee! : Amount.zero(),
                  label: "Sats/vb",
                  controller: _feeController,
                  validator: (value) {
                    if (value == null || value == "0") {
                      return "The fee rate must be greater than 0";
                    }
                    return null;
                  },
                  onChanged: (value) {
                    if (value.isEmpty) {
                      fee = null;
                    }
                    setState(() => fee = Amount.parseAmount(value));
                  },
                ),
                const SizedBox(height: 20),
                Row(
                  mainAxisAlignment: MainAxisAlignment.end,
                  children: [
                    ElevatedButton(
                        onPressed: (_formKey.currentState?.validate() ?? false)
                            ? () async {
                                final messenger = ScaffoldMessenger.of(context);
                                try {
                                  await context
                                      .read<WalletChangeNotifier>()
                                      .service
                                      .sendPayment(address!, amount!, fee!);

                                  setState(() {
                                    _formKey.currentState!.reset();
                                    _addressController.clear();
                                    address = null;
                                    _amountController.clear();
                                    amount = null;
                                    _feeController.clear();
                                    fee = null;

                                    _formKey.currentState!.validate();
                                  });

                                  showSnackBar(messenger, "Payment has been sent.");
                                } catch (e) {
                                  showSnackBar(messenger, "Failed to send payment. $e");
                                }
                              }
                            : null,
                        style: ButtonStyle(
                            padding: WidgetStateProperty.all<EdgeInsets>(const EdgeInsets.all(15)),
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
                                  borderRadius: BorderRadius.circular(10.0),
                                  side: BorderSide(color: tenTenOnePurple.shade100),
                                );
                              } else {
                                return RoundedRectangleBorder(
                                  borderRadius: BorderRadius.circular(10.0),
                                  side: const BorderSide(color: tenTenOnePurple),
                                );
                              }
                            })),
                        child: const Text(
                          "Send",
                          style: TextStyle(fontSize: 18, color: Colors.white),
                        )),
                  ],
                ),
              ]),
            )),
      ],
    );
  }

  @override
  void dispose() {
    super.dispose();

    _addressController.dispose();
    _amountController.dispose();
    _feeController.dispose();
  }
}
