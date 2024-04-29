import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/edit_modal.dart';
import 'package:get_10101/common/intersperse.dart';
import 'package:get_10101/features/wallet/domain/confirmation_target.dart';
import 'package:get_10101/features/wallet/domain/fee.dart';
import 'package:get_10101/features/wallet/domain/fee_estimate.dart';
import 'package:get_10101/features/wallet/send/fee_text.dart';

// TODO: The estimated fee in USD is always 0.

class FeePicker extends StatefulWidget {
  final void Function(FeeConfig) onChange;
  final FeeConfig initialSelection;

  const FeePicker(
      {super.key,
      this.feeEstimates,
      this.customFee,
      required this.onChange,
      required this.initialSelection});
  final Map<ConfirmationTarget, FeeEstimation>? feeEstimates;
  final FeeEstimation? customFee;

  @override
  State<StatefulWidget> createState() => _FeePickerState();
}

class _FeePickerState extends State<FeePicker> {
  late FeeConfig _feeConfig;

  @override
  void initState() {
    super.initState();
    _feeConfig = widget.initialSelection;
  }

  Future<FeeConfig?> _showModal(BuildContext context) => showEditModal<FeeConfig?>(
      context: context,
      builder: (BuildContext context, setVal) => Theme(
            data: Theme.of(context).copyWith(
                textTheme:
                    const TextTheme(labelMedium: TextStyle(fontSize: 16, color: Colors.black)),
                colorScheme: Theme.of(context).colorScheme.copyWith(onSurface: Colors.white)),
            child: _FeePickerModal(
                feeEstimates: widget.feeEstimates,
                customFee: widget.customFee,
                initialSelection: _feeConfig,
                setVal: setVal),
          ));

  @override
  Widget build(BuildContext context) {
    return ElevatedButton(
        onPressed: () {
          _showModal(context).then((val) {
            setState(() => _feeConfig = val ?? _feeConfig);
            widget.onChange(_feeConfig);
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
            Text(_feeConfig.name, style: const TextStyle(fontSize: 16)),
            const Spacer(),
            feeWidget(widget.feeEstimates, widget.customFee, _feeConfig),
            const SizedBox(width: 5),
            const Icon(Icons.arrow_drop_down_outlined, size: 36),
          ],
        ));
  }
}

class _FeePickerModal extends StatefulWidget {
  final FeeConfig initialSelection;
  final Map<ConfirmationTarget, FeeEstimation>? feeEstimates;
  final FeeEstimation? customFee;
  final void Function(FeeConfig?) setVal;

  const _FeePickerModal(
      {this.feeEstimates, this.customFee, required this.initialSelection, required this.setVal});

  @override
  State<StatefulWidget> createState() => _FeePickerModalState();
}

class _FeePickerModalState extends State<_FeePickerModal> {
  late FeeConfig selected;
  final TextEditingController _controller = TextEditingController();

  @override
  void initState() {
    super.initState();
    selected = widget.initialSelection;

    if (selected is CustomFeeRate) {
      _controller.text = (selected as CustomFeeRate).feeRate.toString();
    }
  }

  Widget buildTile(ConfirmationTarget target) {
    bool isSelected = selected is PriorityFee && (selected as PriorityFee).priority == target;

    return TextButton(
      onPressed: () {
        setValue(PriorityFee(target));
        Navigator.pop(context);
      },
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
              feeWidget(widget.feeEstimates, widget.customFee, PriorityFee(target)),
            ],
          ),
        ),
      ),
    );
  }

  void setValue(FeeConfig feeConfig) => setState(() {
        selected = feeConfig;
        widget.setVal(selected);
      });

  void setCustomValue({String? val}) {
    val = val ?? _controller.text;
    if (validateCustomValue(val) == null) {
      setValue(CustomFeeRate(feeRate: int.parse(val)));
    }
  }

  int get minFee => widget.feeEstimates?[ConfirmationTarget.minimum]?.total.sats ?? 0;

  String? validateCustomValue(String? val) {
    if (val == null) {
      return "Enter a value";
    }

    final amt = Amount.parseAmount(val);

    if (amt.sats < 1) {
      return "The minimum fee to broadcast the transaction is 1 sat/vByte.";
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
        TextButton(
            onPressed: () => showDialog<void>(
                context: context,
                builder: (BuildContext context) => AlertDialog(
                      title: const Text('Fee (sats/vByte)', style: TextStyle(color: Colors.black)),
                      content: TextField(
                        style: const TextStyle(color: Colors.black),
                        controller: _controller,
                        inputFormatters: [FilteringTextInputFormatter.digitsOnly],
                      ),
                      actions: [
                        TextButton(
                          child: const Text('Cancel', style: TextStyle(color: Colors.black)),
                          onPressed: () {
                            Navigator.pop(context);
                          },
                        ),
                        TextButton(
                          child: const Text('OK', style: TextStyle(color: Colors.black)),
                          onPressed: () {
                            int feeRate = int.tryParse(_controller.text)!;
                            widget.setVal(CustomFeeRate(feeRate: feeRate));

                            Navigator.pop(context);
                            Navigator.pop(context);
                          },
                        )
                      ],
                    )),
            child: const Text('Custom')),
        const SizedBox(height: 25),
      ],
    );
  }
}

Widget feeWidget(Map<ConfirmationTarget, FeeEstimation>? feeEstimates, FeeEstimation? customFee,
    FeeConfig feeConfig) {
  return switch (feeConfig) {
    PriorityFee() => switch (feeEstimates?[(feeConfig).priority]) {
        null => const SizedBox.square(dimension: 24, child: CircularProgressIndicator()),
        var fee => FeeText(fee: fee),
      },
    CustomFeeRate() => FeeText(
        fee: FeeEstimation(
            satsPerVbyte: customFee!.satsPerVbyte.toDouble(), total: customFee.total)),
  };
}
