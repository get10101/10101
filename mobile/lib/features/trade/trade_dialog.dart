import 'dart:convert';
import 'dart:io';

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/domain/background_task.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/global_keys.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/common/task_status_dialog.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/brag/brag.dart';
import 'package:get_10101/features/trade/domain/trade_values.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:provider/provider.dart';
import 'package:share_plus/share_plus.dart';
import 'package:social_share/social_share.dart';
import 'package:url_launcher/url_launcher.dart';

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
    children.addAll(
      [
        SizedBox(
          width: 200,
          child: Wrap(
            runSpacing: 10,
            children: [
              pendingOrder.positionAction == PositionAction.close
                  ? ValueDataRow(type: ValueType.amount, value: pendingOrder.pnl, label: pnlText)
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
      children.add(
        const Padding(
          padding: EdgeInsets.only(left: 10, right: 10, bottom: 5),
          child: Text("Do not close the app!",
              style: TextStyle(fontSize: 15, fontWeight: FontWeight.bold)),
        ),
      );
    }

    // Only display "share on twitter" when order is filled
    if (pendingOrder.state == PendingOrderState.orderFilled) {
      children.add(Padding(
        padding: const EdgeInsets.only(top: 20, left: 10, right: 10, bottom: 5),
        child: ElevatedButton(
            onPressed: () async {
              showDialog(
                context: context,
                builder: (BuildContext context) {
                  double realizedPnl = double.parse(pendingOrder.pnl?.sats.toString() ?? "0");
                  double margin = double.parse(pendingOrderValues?.margin?.sats.toString() ?? "0");
                  double pnlPercent = (realizedPnl / margin) * 100.0;
                  return BragWidget(
                    title: 'Share as image',
                    onClose: () {
                      Navigator.of(context).pop();
                    },
                    direction: pendingOrderValues!.direction,
                    leverage: pendingOrderValues.leverage,
                    pnl: pendingOrder.pnl ?? Amount.zero(),
                    pnlPercent: double.parse(pnlPercent.toStringAsFixed(0)).toInt(),
                    entryPrice: Usd.fromDouble(pendingOrderValues.price ?? 0.0),
                  );
                },
              );
            },
            child: const Text("Share as image")),
      ));
    }
  }

  return Column(
    mainAxisSize: MainAxisSize.min,
    children: children,
  );
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

class ClickableHelpText extends StatelessWidget {
  const ClickableHelpText({super.key});

  @override
  Widget build(BuildContext context) {
    return RichText(
      text: TextSpan(
        text: 'Please help us fix this issue and join our telegram group: ',
        style: DefaultTextStyle.of(context).style,
        children: [
          TextSpan(
            text: 'https://t.me/get10101',
            style: const TextStyle(
              color: Colors.blue,
              decoration: TextDecoration.underline,
            ),
            recognizer: TapGestureRecognizer()
              ..onTap = () async {
                final httpsUri = Uri(scheme: 'https', host: 't.me', path: 'get10101');
                if (await canLaunchUrl(httpsUri)) {
                  await launchUrl(httpsUri, mode: LaunchMode.externalApplication);
                } else {
                  showSnackBar(ScaffoldMessenger.of(rootNavigatorKey.currentContext!),
                      "Failed to open link");
                }
              },
          ),
        ],
      ),
    );
  }
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
          const ClickableHelpText(),
        ],
      ),
    );
  }
}
