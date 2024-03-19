import 'dart:io';

import 'package:flutter/material.dart';
import 'package:get_10101/common/domain/background_task.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/trade/domain/trade_values.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:get_10101/features/wallet/receive/receive_usdp_status_dialog.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:provider/provider.dart';
import 'package:share_plus/share_plus.dart';
import 'package:social_share/social_share.dart';

class ReceiveUsdpDialog extends StatelessWidget {
  const ReceiveUsdpDialog({super.key});

  @override
  Widget build(BuildContext context) {
    final submitOrderChangeNotifier = context.watch<SubmitOrderChangeNotifier>();
    final pendingOrder = submitOrderChangeNotifier.pendingOrder!;
    final pendingOrderValues = submitOrderChangeNotifier.pendingOrderValues;

    Widget body = createSubmitWidget(pendingOrder, pendingOrderValues, context);

    switch (pendingOrder.state) {
      case PendingOrderState.submitting:
      case PendingOrderState.submittedSuccessfully:
        return ReceiveUsdpTaskStatusDialog(
            title: "Converting your received sats to USDP",
            status: TaskStatus.pending,
            content: body);
      case PendingOrderState.submissionFailed:
        return ReceiveUsdpTaskStatusDialog(
            title: "Submit Order", status: TaskStatus.failed, content: body);
      case PendingOrderState.orderFilled:
        return ReceiveUsdpTaskStatusDialog(
          title: "You received USDP",
          status: TaskStatus.success,
          content: body,
          buttonText: "Awesome 🥳",
          navigateToRoute: WalletScreen.route,
        );
      case PendingOrderState.orderFailed:
        return ReceiveUsdpTaskStatusDialog(
          title: "Couldn't convert to USDP",
          status: TaskStatus.failed,
          content: body,
          buttonText: "Oh snap 😕",
        );
    }
  }
}

Widget createSubmitWidget(
    PendingOrder pendingOrder, TradeValues? pendingOrderValues, BuildContext context) {
  String bottomText;

  switch (pendingOrder.state) {
    case PendingOrderState.submittedSuccessfully:
    case PendingOrderState.submitting:
      bottomText = "Please wait while your sats are converted to USDP.";
      break;
    case PendingOrderState.orderFailed:
    case PendingOrderState.submissionFailed:
      bottomText = "Sorry, we couldn't match your order. Please try again later.";
      break;
    case PendingOrderState.orderFilled:
      var amount = pendingOrder.tradeValues?.quantity?.toInt ?? "0";
      bottomText = "Congratulations! You received $amount USDP.";
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
                type: ValueType.fiat, value: pendingOrderValues?.quantity?.toInt, label: "USDP"),
            ValueDataRow(
                type: ValueType.amount, value: pendingOrderValues?.margin, label: "Margin"),
            ValueDataRow(
                type: ValueType.amount, value: pendingOrderValues?.fee ?? Amount(0), label: "Fee")
          ],
        ),
      ),
      Padding(
        padding: const EdgeInsets.only(top: 20, left: 10, right: 10, bottom: 5),
        child: Center(
            child: Text(
          bottomText,
          style: const TextStyle(
            fontSize: 15,
          ),
          textAlign: TextAlign.center,
        )),
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
  String shareText =
      "I just witnessed the future and received USD-P via lightning using #DLC with @get10101 🚀. The future of decentralised finance starts now! #Bitcoin";

  if (Platform.isAndroid || Platform.isIOS) {
    await SocialShare.shareTwitter(shareText);
  } else {
    await Share.share(shareText);
  }
}
