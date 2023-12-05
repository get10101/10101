import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:intl/intl.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/amount_text_input_form_field.dart';
import 'package:get_10101/common/domain/model.dart';

void _doNothing(int? v) {}

enum BtcOrFiat {
  btc,
  fiat,
}

String format(BtcOrFiat type, int amount) {
  final formatter = NumberFormat("#,###,##0.00", "en");
  return type == BtcOrFiat.btc ? formatSats(Amount(amount)) : "\$${formatter.format(amount)}";
}

class AmountInputModalisedField extends StatelessWidget {
  final void Function(int?) onChange;
  final String? Function(String?)? validator;
  final BtcOrFiat type;
  final int? amount;
  const AmountInputModalisedField(
      {super.key, this.onChange = _doNothing, required this.type, this.amount, this.validator});

  @override
  Widget build(BuildContext context) {
    return FormField<int?>(
        initialValue: amount,
        validator: validator != null ? (amt) => validator!(amt.toString()) : null,
        builder: (FormFieldState<int?> field) {
          onValueChange(int? val) {
            onChange(val);
            field.didChange(val);
          }

          return InputDecorator(
            decoration: InputDecoration(
                errorText: field.errorText,
                border: InputBorder.none,
                isDense: true,
                contentPadding: EdgeInsets.zero),
            child: OutlinedButton(
                onPressed: () => _showModal(context, type, amount, onValueChange, validator),
                style: OutlinedButton.styleFrom(
                  minimumSize: const Size(20, 50),
                  backgroundColor: Colors.white,
                  shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
                ),
                child: Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                    Text(
                      amount != null ? format(type, amount!) : "Set amount",
                      style: const TextStyle(color: Colors.black87, fontSize: 16),
                    ),
                    const Icon(Icons.edit, size: 20)
                  ],
                )),
          );
        });
  }
}

void _showModal(BuildContext context, BtcOrFiat type, int? amount, void Function(int?) onSetAmount,
    String? Function(String?)? validator) {
  showModalBottomSheet<void>(
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(
          top: Radius.circular(20),
        ),
      ),
      clipBehavior: Clip.antiAlias,
      isScrollControlled: true,
      useRootNavigator: true,
      context: context,
      builder: (BuildContext context) {
        return SafeArea(
            child: Padding(
                padding: EdgeInsets.only(bottom: MediaQuery.of(context).viewInsets.bottom),
                // the GestureDetector ensures that we can close the keyboard by tapping into the modal
                child: GestureDetector(
                  onTap: () {
                    FocusScopeNode currentFocus = FocusScope.of(context);

                    if (!currentFocus.hasPrimaryFocus) {
                      currentFocus.unfocus();
                    }
                  },
                  child: SingleChildScrollView(
                    child: SizedBox(
                      // TODO: Find a way to make height dynamic depending on the children size
                      // This is needed because otherwise the keyboard does not push the sheet up correctly
                      height: 200,
                      child: EnterAmountModal(
                          amount: amount,
                          onSetAmount: onSetAmount,
                          validator: validator,
                          type: type),
                    ),
                  ),
                )));
      });
}

class EnterAmountModal extends StatefulWidget {
  final int? amount;
  final BtcOrFiat type;
  final void Function(int?) onSetAmount;
  final String? Function(String?)? validator;

  const EnterAmountModal(
      {super.key, this.amount, required this.onSetAmount, required this.type, this.validator});

  @override
  State<EnterAmountModal> createState() => _EnterAmountModalState();
}

class _EnterAmountModalState extends State<EnterAmountModal> {
  int? amount = 0;
  final _formKey = GlobalKey<FormState>();

  @override
  void initState() {
    super.initState();
    amount = widget.amount;
  }

  @override
  Widget build(BuildContext context) {
    String hint = widget.type == BtcOrFiat.btc ? formatSats(Amount(50000)) : "\$100";

    return Padding(
      padding: const EdgeInsets.only(left: 20.0, top: 30.0, right: 20.0),
      child: Form(
        key: _formKey,
        autovalidateMode: AutovalidateMode.always,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            AmountInputField(
              value: widget.amount != null ? Amount(widget.amount!) : Amount.zero(),
              hint: "e.g. $hint",
              label: "Amount",
              validator: widget.validator,
              onChanged: (value) {
                if (value.isEmpty) {
                  amount = null;
                }
                amount = Amount.parseAmount(value).sats;
              },
            ),
            const SizedBox(height: 20),
            ElevatedButton(
                onPressed: () {
                  if (_formKey.currentState!.validate()) {
                    widget.onSetAmount(amount);
                    GoRouter.of(context).pop();
                  }
                },
                child: const Text("Set Amount", style: TextStyle(fontSize: 16)))
          ],
        ),
      ),
    );
  }
}
