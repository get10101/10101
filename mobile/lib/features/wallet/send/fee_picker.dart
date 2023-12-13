import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_and_fiat_text.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/amount_text_input_form_field.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/edit_modal.dart';
import 'package:get_10101/common/intersperse.dart';
import 'package:get_10101/features/wallet/domain/confirmation_target.dart';
import 'package:get_10101/features/wallet/domain/fee.dart';

class FeePicker extends StatefulWidget {
  final void Function(Fee) onChange;
  final Fee initialSelection;

  const FeePicker(
      {super.key, this.feeAmounts, required this.onChange, required this.initialSelection});
  final Map<ConfirmationTarget, int>? feeAmounts;

  @override
  State<StatefulWidget> createState() => _FeePickerState();
}

class _FeePickerState extends State<FeePicker> {
  late Fee _fee;

  @override
  void initState() {
    super.initState();
    _fee = widget.initialSelection;
  }

  Future<Fee?> _showModal(BuildContext context) => showEditModal<Fee?>(
      context: context,
      builder: (BuildContext context, setVal) => Theme(
            data: Theme.of(context).copyWith(
                textTheme:
                    const TextTheme(labelMedium: TextStyle(fontSize: 16, color: Colors.black)),
                colorScheme: Theme.of(context).colorScheme.copyWith(background: Colors.white)),
            child: _FeePickerModal(
                feeAmounts: widget.feeAmounts, initialSelection: _fee, setVal: setVal),
          ));

  @override
  Widget build(BuildContext context) {
    int feeSats = switch (_fee) {
      PriorityFee() => widget.feeAmounts?[(_fee as PriorityFee).priority] ?? -1,
      CustomFee() => (_fee as CustomFee).amount.sats,
    };

    final fees = switch (feeSats) {
      -1 => const SizedBox.square(dimension: 24, child: CircularProgressIndicator()),
      int() => AmountAndFiatText(amount: Amount(feeSats))
    };

    return ElevatedButton(
        onPressed: () {
          _showModal(context).then((val) {
            setState(() => _fee = val ?? _fee);
            widget.onChange(_fee);
          });
        },
        style: ElevatedButton.styleFrom(
          minimumSize: const Size(20, 50),
          shadowColor: Colors.transparent,
          backgroundColor: Colors.orange.shade300.withOpacity(0.1),
          foregroundColor: Colors.black,
          textStyle: const TextStyle(),
          shape: RoundedRectangleBorder(
              side: BorderSide(color: Colors.grey.shade200),
              borderRadius: BorderRadius.circular(10)),
          padding: const EdgeInsets.only(left: 25, top: 25, bottom: 25, right: 10),
        ),
        child: Row(
          children: [
            Text(_fee.name, style: const TextStyle(fontSize: 16)),
            const Spacer(),
            fees,
            const SizedBox(width: 5),
            const Icon(Icons.arrow_drop_down_outlined, size: 36),
          ],
        ));
  }
}

class _FeePickerModal extends StatefulWidget {
  final Fee initialSelection;
  final Map<ConfirmationTarget, int>? feeAmounts;
  final void Function(Fee?) setVal;

  const _FeePickerModal({this.feeAmounts, required this.initialSelection, required this.setVal});

  @override
  State<StatefulWidget> createState() => _FeePickerModalState();
}

class _FeePickerModalState extends State<_FeePickerModal> {
  late Fee selected;
  final TextEditingController _controller = TextEditingController();
  final _formKey = GlobalKey<FormState>();

  @override
  void initState() {
    super.initState();
    selected = widget.initialSelection;

    if (selected is CustomFee) {
      _controller.text = (selected as CustomFee).amount.formatted();
    }
  }

  Widget buildTile(ConfirmationTarget target) {
    Widget fee = switch (widget.feeAmounts) {
      Map() => AmountAndFiatText(amount: Amount(widget.feeAmounts![target]!)),
      null => const SizedBox.square(dimension: 24, child: CircularProgressIndicator()),
    };

    bool isSelected = selected is PriorityFee && (selected as PriorityFee).priority == target;

    return TextButton(
      onPressed: () => setValue(PriorityFee(target)),
      style: TextButton.styleFrom(foregroundColor: Colors.orange.shade300.withOpacity(0.1)),
      child: DefaultTextStyle(
        style: Theme.of(context).textTheme.labelMedium!,
        child: Padding(
          padding: const EdgeInsets.all(20),
          child: Row(
            children: [
              SizedBox.square(
                  dimension: 22,
                  child: Visibility(
                      visible: isSelected,
                      child: const Icon(Icons.check, size: 22, color: Colors.black))),
              const SizedBox(width: 8),
              Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
                Text(target.toString()),
                Text(target.toTimeEstimate(), style: const TextStyle(color: Color(0xff878787))),
              ]),
              const Spacer(),
              fee,
            ],
          ),
        ),
      ),
    );
  }

  void setValue(Fee fee) => setState(() {
        selected = fee;
        widget.setVal(selected);
      });

  void setCustomValue({String? val}) {
    val = val ?? _controller.text;
    if (validateCustomValue(val) == null) {
      setValue(CustomFee(amount: Amount.parseAmount(val)));
    }
  }

  int get minFee => widget.feeAmounts?[ConfirmationTarget.minimum] ?? 0;

  String? validateCustomValue(String? val) {
    if (val == null) {
      return "Enter a value";
    }

    final amt = Amount.parseAmount(val);

    if (minFee > amt.sats) {
      return "The minimum fee to broadcast the transaction is ${formatSats(Amount(minFee))}.";
    }

    return null;
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const SizedBox(height: 20),
        ...ConfirmationTarget.options
            .map(buildTile)
            .intersperse(const Divider(height: 0.5, thickness: 0.5)),
        const SizedBox(height: 25),
        const Padding(
          padding: EdgeInsets.only(left: 25, bottom: 10),
          child: Text("Custom", style: TextStyle(color: Colors.grey)),
        ),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 25),
          child: Form(
            key: _formKey,
            autovalidateMode: AutovalidateMode.onUserInteraction,
            child: AmountInputField(
                controller: _controller,
                onChanged: (val) => setCustomValue(val: val),
                validator: validateCustomValue,
                onTap: () => setCustomValue(),
                style: const TextStyle(color: Colors.black, fontSize: 20),
                decoration: InputDecoration(
                    hintText: minFee.toString(),
                    border: OutlineInputBorder(
                        borderSide: BorderSide.none, borderRadius: BorderRadius.circular(10)),
                    fillColor: const Color(0xfff4f4f4),
                    filled: true,
                    errorStyle: TextStyle(
                      color: Colors.red[900],
                    ),
                    errorMaxLines: 3,
                    suffix: const Text(
                      "sats",
                      style: TextStyle(fontSize: 16, color: Color(0xff878787)),
                    )),
                value: Amount(1)),
          ),
        ),
        const SizedBox(height: 25),
        Padding(
          padding: const EdgeInsets.all(20),
          child: OutlinedButton(
              onPressed: () => Navigator.pop(context, selected),
              style: OutlinedButton.styleFrom(
                  padding: EdgeInsets.zero,
                  side: const BorderSide(color: tenTenOnePurple),
                  shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12))),
              child: const Padding(
                padding: EdgeInsets.all(16.0),
                child: Text("Done", style: TextStyle(fontWeight: FontWeight.normal, fontSize: 20)),
              )),
        )
      ],
    );
  }
}
