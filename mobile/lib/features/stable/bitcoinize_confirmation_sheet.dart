import 'package:flutter/material.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/stable/stable_dialog.dart';
import 'package:get_10101/features/stable/stable_value_change_notifier.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/position.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

bitcoinizeBottomSheet({required BuildContext context, required Position position}) {
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
              height: 300,
              child: BitcoinizeBottomSheet(position: position),
            ),
          ),
        ),
      ));
    },
  );
}

class BitcoinizeBottomSheet extends StatelessWidget {
  final Position position;

  const BitcoinizeBottomSheet({super.key, required this.position});

  @override
  Widget build(BuildContext context) {
    final stableValuesChangeNotifier = context.watch<StableValuesChangeNotifier>();

    final stableValues = stableValuesChangeNotifier.stableValues();
    stableValues.quantity = position.quantity;
    stableValues.direction = Direction.long;

    return Container(
      padding: const EdgeInsets.all(20),
      child: Column(
        children: [
          const Text(
            "Bitcoinize your synthetic USD?",
            style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold),
          ),
          const SizedBox(height: 16.0),
          ValueDataRow(
              type: ValueType.fiat,
              value: position.quantity,
              label: 'You have',
              valueTextStyle: const TextStyle(fontSize: 18),
              labelTextStyle: const TextStyle(fontSize: 18)),
          const SizedBox(height: 16.0),
          ValueDataRow(
              type: ValueType.amount,
              value: position.getAmountWithUnrealizedPnl(),
              label: 'You will receive',
              valueTextStyle: const TextStyle(fontSize: 18),
              labelTextStyle: const TextStyle(fontSize: 18)),
          const SizedBox(height: 16.0),
          ValueDataRow(
              type: ValueType.amount,
              value: stableValues.fee,
              label: 'Fees',
              valueTextStyle: const TextStyle(fontSize: 18),
              labelTextStyle: const TextStyle(fontSize: 18)),
          const SizedBox(height: 16.0),
          const Text("Your synthetic USD will be converted back into Sats"),
          Expanded(
              child: Column(
            mainAxisAlignment: MainAxisAlignment.end,
            children: [
              ElevatedButton(
                  onPressed: () {
                    final submitOrderChangeNotifier = context.read<SubmitOrderChangeNotifier>();
                    stableValues.margin = position.getAmountWithUnrealizedPnl();
                    submitOrderChangeNotifier.submitPendingOrder(
                        stableValues, PositionAction.close);

                    // Return to the stable screen before submitting the pending order so that the dialog is displayed under the correct context
                    GoRouter.of(context).pop();

                    showDialog(
                        context: context,
                        useRootNavigator: true,
                        barrierDismissible: false, // Prevent user from leaving
                        builder: (BuildContext context) {
                          return const StableDialog();
                        });
                  },
                  child: const Text("Confirm")),
            ],
          ))
        ],
      ),
    );
  }
}
