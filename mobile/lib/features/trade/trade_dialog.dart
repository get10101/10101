import 'dart:io';

import 'package:flutter/material.dart';
import 'package:get_10101/common/domain/background_task.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/task_status_dialog.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/domain/trade_values.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:provider/provider.dart';
import 'package:share_plus/share_plus.dart';
import 'package:social_share/social_share.dart';

class TradeDialog extends StatelessWidget {
  const TradeDialog({super.key});

  @override
  Widget build(BuildContext context) {
    Amount pnl = Amount(0);
    final submitOrderChangeNotifier = context.watch<SubmitOrderChangeNotifier>();
    final pendingOrder = submitOrderChangeNotifier.pendingOrder!;
    final pendingOrderValues = submitOrderChangeNotifier.pendingOrderValues;

    if (pendingOrder.positionAction == PositionAction.close &&
        context.read<PositionChangeNotifier>().positions.containsKey(ContractSymbol.btcusd)) {
      final position = context.read<PositionChangeNotifier>().positions[ContractSymbol.btcusd];
      pnl = position!.unrealizedPnl != null ? position.unrealizedPnl! : Amount(0);
    }

    Widget body = createSubmitWidget(pendingOrder, pnl, pendingOrderValues, context);

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
        return TaskStatusDialog(title: "Fill Order", status: TaskStatus.failed, content: body);
    }
  }
}

Widget createSubmitWidget(
    PendingOrder pendingOrder, Amount pnl, TradeValues? pendingOrderValues, BuildContext context) {
  String bottomText;
  String pnlText = "P/L";

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
        bottomText = "Your position has been closed.";
        // At this point, the position is closed so P/L has been realized
        // TODO - calculate based on subchannel finalized event
        pnlText = "P/L";
      } else {
        bottomText = "Congratulations! Your position will be shown in the Positions tab.";
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
            pendingOrder.positionAction == PositionAction.close
                ? ValueDataRow(type: ValueType.amount, value: pnl, label: pnlText)
                : ValueDataRow(
                    type: ValueType.amount, value: pendingOrderValues?.margin, label: "Margin"),
            ValueDataRow(
                type: ValueType.amount, value: pendingOrderValues?.fee ?? Amount(0), label: "Fee")
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
    body.children.add(
      const Padding(
        padding: EdgeInsets.only(left: 10, right: 10, bottom: 5),
        child: Text("Do not close the app!",
            style: TextStyle(fontSize: 15, fontWeight: FontWeight.bold)),
      ),
    );
  }

  // Only display "share on twitter" when order is filled
  if (pendingOrder.state == PendingOrderState.orderFilled) {
    body.children.add(Padding(
      padding: const EdgeInsets.only(top: 20, left: 10, right: 10, bottom: 5),
      child: ElevatedButton(
          onPressed: () async {
            await shareTweet(pendingOrder.positionAction);
          },
          child: const Text("Share on Twitter")),
    ));
  }

  return body;
}

Future<void> shareTweet(PositionAction action) async {
  String actionStr = action == PositionAction.open ? "opened" : "closed";
  String shareText =
      "Just $actionStr a #selfcustodial position using #DLC with @get10101 🚀. The future of decentralised finance starts now! #Bitcoin";

  if (Platform.isAndroid || Platform.isIOS) {
    await SocialShare.shareTwitter(shareText);
  } else {
    await Share.share(shareText);
  }
}
