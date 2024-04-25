import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/application/clickable_help_text.dart';
import 'package:get_10101/common/application/tentenone_config_change_notifier.dart';
import 'package:get_10101/common/domain/background_task.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/task_status_dialog.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/trade/domain/trade_values.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:provider/provider.dart';
import 'package:share_plus/share_plus.dart';

class TradeDialog extends StatelessWidget {
  const TradeDialog({super.key});

  @override
  Widget build(BuildContext context) {
    final submitOrderChangeNotifier = context.watch<SubmitOrderChangeNotifier>();
    final pendingOrder = submitOrderChangeNotifier.pendingOrder!;
    final pendingOrderValues = submitOrderChangeNotifier.pendingOrderValues;

    Widget body = createSubmitWidget(pendingOrder, pendingOrderValues, context);

    switch (pendingOrder.state) {
      case PendingOrderState.submitting:
        return TaskStatusDialog(title: "Submit Order", status: TaskStatus.pending, content: body);
      case PendingOrderState.submittedSuccessfully:
        return TaskStatusDialog(title: "Fill Order", status: TaskStatus.pending, content: body);
      case PendingOrderState.submissionFailed:
        return TaskStatusDialog(title: "Submit Order", status: TaskStatus.failed, content: body);
      case PendingOrderState.orderFilled:
        return TaskStatusDialog(title: "Fill Order", status: TaskStatus.success, content: body);
      case PendingOrderState.orderFailed:
        return TaskStatusDialog(title: "Order", status: TaskStatus.failed, content: body);
    }
  }
}

Widget createSubmitWidget(
    PendingOrder pendingOrder, TradeValues? pendingOrderValues, BuildContext context) {
  String bottomText;
  String pnlText = "P/L";

  switch (pendingOrder.state) {
    case PendingOrderState.submittedSuccessfully:
    case PendingOrderState.submitting:
      bottomText = "Please wait while the order is being processed.";
      break;
    case PendingOrderState.orderFailed:
    case PendingOrderState.submissionFailed:
      bottomText = "Submission failed: ${pendingOrder.submitOrderError}.";
      break;
    case PendingOrderState.orderFilled:
      if (pendingOrder.positionAction == PositionAction.close) {
        bottomText = "Your position has been closed.";
        // At this point, the position is closed so P/L has been realized
        // TODO - calculate based on subchannel finalized event
        pnlText = "P/L";
      } else {
        bottomText = "Congratulations! Your position will be shown in the Positions tab.";
      }
      break;
  }

  List<Widget> children = [];
  if (pendingOrder.failureReason != null) {
    children.add(
      ErrorDetails(
        details: pendingOrder.failureReason!.details ?? "unknown error",
      ),
    );
  } else {
    Amount fee = pendingOrderValues?.fee ?? Amount.zero();
    final referralStatus = context.read<TenTenOneConfigChangeNotifier>().referralStatus;
    if (referralStatus != null) {
      final feeRebate = fee.sats * referralStatus.referralFeeBonus;
      fee -= Amount(feeRebate.floor());
    }

    children.addAll(
      [
        SizedBox(
          width: 200,
          child: Wrap(
            runSpacing: 5,
            children: [
              pendingOrder.positionAction == PositionAction.close
                  ? ValueDataRow(type: ValueType.amount, value: pendingOrder.pnl, label: pnlText)
                  : ValueDataRow(
                      type: ValueType.amount, value: pendingOrderValues?.margin, label: "Margin"),
              ValueDataRow(type: ValueType.amount, value: fee, label: "Fee")
            ],
          ),
        ),
        Padding(
          padding: const EdgeInsets.only(top: 20, left: 10, right: 10, bottom: 5),
          child: Text(bottomText, style: const TextStyle(fontSize: 15)),
        ),
      ],
    );

    // Add "Do not close the app" while order is pending
    if (pendingOrder.state == PendingOrderState.submitting ||
        pendingOrder.state == PendingOrderState.submittedSuccessfully) {
      children.add(
        const Padding(
          padding: EdgeInsets.only(left: 10, right: 10, bottom: 5),
          child: Text("Do not close the app!",
              style: TextStyle(fontSize: 15, fontWeight: FontWeight.bold)),
        ),
      );
    }
  }

  return Column(
    mainAxisSize: MainAxisSize.min,
    children: children,
  );
}

// Returns a formatted json string if the provided argument is json, else, returns the argument
String getPrettyJSONString(String jsonObjectString) {
  try {
    var jsonObject = json.decode(jsonObjectString);
    var encoder = const JsonEncoder.withIndent("     ");
    return encoder.convert(jsonObject);
  } catch (error) {
    return jsonObjectString;
  }
}

class ErrorDetails extends StatelessWidget {
  final String details;

  const ErrorDetails({super.key, required this.details});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(top: 20, left: 10, right: 10, bottom: 5),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Text(
            "Error details:",
            style: TextStyle(fontSize: 15),
          ),
          Padding(
            padding: const EdgeInsets.all(5.0),
            child: SizedBox.square(
              child: Container(
                padding: const EdgeInsets.fromLTRB(5, 25, 5, 10.0),
                color: Colors.grey.shade300,
                child: Column(
                  children: [
                    Text(
                      getPrettyJSONString(details),
                      style: const TextStyle(fontSize: 15),
                    ),
                    Row(
                      mainAxisAlignment: MainAxisAlignment.end,
                      crossAxisAlignment: CrossAxisAlignment.end,
                      children: [
                        GestureDetector(
                          child: const Icon(Icons.content_copy, size: 16),
                          onTap: () {
                            Clipboard.setData(ClipboardData(text: details)).then((_) {
                              ScaffoldMessenger.of(context).showSnackBar(
                                const SnackBar(
                                  content: Text("Copied to clipboard"),
                                ),
                              );
                            });
                          },
                        ),
                        Padding(
                          padding: const EdgeInsets.only(
                            left: 8.0,
                            right: 8.0,
                          ),
                          child: GestureDetector(
                            child: const Icon(Icons.share, size: 16),
                            onTap: () => Share.share(details),
                          ),
                        )
                      ],
                    ),
                  ],
                ),
              ),
            ),
          ),
          ClickableHelpText(
              text: "Please help us fix this issue and join our telegram group: ",
              style: DefaultTextStyle.of(context).style),
        ],
      ),
    );
  }
}
