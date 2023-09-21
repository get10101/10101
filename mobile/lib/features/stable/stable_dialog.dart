import 'package:flutter/material.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/stable/stable_submission_status_dialog.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:provider/provider.dart';

class StableDialog extends StatelessWidget {
  const StableDialog({super.key});

  @override
  Widget build(BuildContext context) {
    final submitOrderChangeNotifier = context.watch<SubmitOrderChangeNotifier>();
    final pendingOrder = submitOrderChangeNotifier.pendingOrder!;

    Widget body = createSubmitWidget(pendingOrder, DefaultTextStyle.of(context).style);

    switch (pendingOrder.state) {
      case PendingOrderState.submitting:
        return StableSubmissionStatusDialog(
            title:
                pendingOrder.positionAction == PositionAction.open ? "Stabilizing" : "Bitcoinizing",
            type: StableSubmissionStatusDialogType.pendingSubmit,
            content: body);
      case PendingOrderState.submittedSuccessfully:
        return StableSubmissionStatusDialog(
            title: pendingOrder.positionAction == PositionAction.open ? "Stabilize" : "Bitcoinize",
            type: StableSubmissionStatusDialogType.successfulSubmit,
            content: body);
      case PendingOrderState.submissionFailed:
        // TODO: This failure case has to be handled differently; are we planning to show orders that failed to submit in the order history?
        return StableSubmissionStatusDialog(
            title:
                pendingOrder.positionAction == PositionAction.open ? "Stabilizing" : "Bitcoinizing",
            type: StableSubmissionStatusDialogType.failedSubmit,
            content: body);
      case PendingOrderState.orderFilled:
        return StableSubmissionStatusDialog(
            title: pendingOrder.positionAction == PositionAction.open ? "Stabilize" : "Bitcoinize",
            type: StableSubmissionStatusDialogType.filled,
            content: body);
      case PendingOrderState.orderFailed:
        return StableSubmissionStatusDialog(
            title:
                pendingOrder.positionAction == PositionAction.open ? "Stabilizing" : "Bitcoinizing",
            type: StableSubmissionStatusDialogType.failedFill,
            content: body);
    }
  }
}

Widget createSubmitWidget(PendingOrder pendingOrder, TextStyle style) {
  String bottomText;

  switch (pendingOrder.state) {
    case PendingOrderState.submittedSuccessfully:
    case PendingOrderState.submitting:
      bottomText = "Please wait while the order is being processed.";
      break;
    case PendingOrderState.orderFailed:
    case PendingOrderState.submissionFailed:
      bottomText = "Sorry, we couldn't match your order. Please try again later.";
      break;
    case PendingOrderState.orderFilled:
      if (pendingOrder.positionAction == PositionAction.close) {
        bottomText = "Congratulations! Your synthetic USD have been bitcoinized.";
      } else {
        bottomText = "Congratulations! Your sats have been stabilized.";
      }
      break;
  }

  Column body = Column(
    mainAxisSize: MainAxisSize.min,
    children: [
      SizedBox(
        width: 200,
        child: Wrap(
          runSpacing: 10,
          children: [
            ValueDataRow(
                type: pendingOrder.positionAction == PositionAction.close
                    ? ValueType.text
                    : ValueType.amount,
                value: pendingOrder.positionAction == PositionAction.close
                    ? "${pendingOrder.tradeValues?.quantity!.ceil()} \$"
                    : pendingOrder.tradeValues?.margin,
                label: pendingOrder.positionAction == PositionAction.open
                    ? "Stabilize"
                    : "Bitcoinize"),
            ValueDataRow(
                type: pendingOrder.positionAction == PositionAction.open
                    ? ValueType.text
                    : ValueType.amount,
                value: pendingOrder.positionAction == PositionAction.open
                    ? "${pendingOrder.tradeValues?.quantity!.ceil()} \$"
                    : pendingOrder.tradeValues?.margin,
                label: "Into"),
            ValueDataRow(type: ValueType.amount, value: pendingOrder.tradeValues?.fee, label: "Fee")
          ],
        ),
      ),
      Padding(
        padding: const EdgeInsets.only(top: 20, left: 10, right: 10, bottom: 5),
        child: Text(bottomText, style: style.apply(fontSizeFactor: 1.0)),
      ),
    ],
  );

  // Add "Do not close the app" while order is pending
  if (pendingOrder.state == PendingOrderState.submitting ||
      pendingOrder.state == PendingOrderState.submittedSuccessfully) {
    body.children.add(
      Padding(
        padding: const EdgeInsets.only(left: 10, right: 10, bottom: 5),
        child: Text("Do not close the app!",
            style: style.apply(fontSizeFactor: 1.0, fontWeightDelta: 1)),
      ),
    );
  }

  return body;
}
