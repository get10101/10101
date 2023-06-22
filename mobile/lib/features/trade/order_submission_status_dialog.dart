import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

enum OrderSubmissionStatusDialogType {
  pendingSubmit,
  successfulSubmit,
  filled,
  failedFill,
  failedSubmit
}

class OrderSubmissionStatusDialog extends StatelessWidget {
  final String title;
  final OrderSubmissionStatusDialogType type;
  final Widget content;
  final String buttonText;
  final EdgeInsets insetPadding;
  final String navigateToRoute;

  const OrderSubmissionStatusDialog(
      {super.key,
      required this.title,
      required this.type,
      required this.content,
      this.buttonText = "Close",
      this.insetPadding = const EdgeInsets.all(50),
      this.navigateToRoute = ""});

  @override
  Widget build(BuildContext context) {
    bool isPending = type == OrderSubmissionStatusDialogType.successfulSubmit ||
        type == OrderSubmissionStatusDialogType.pendingSubmit;

    Widget closeButton = ElevatedButton(
        onPressed: () {
          GoRouter.of(context).pop();

          if (navigateToRoute.isNotEmpty) {
            GoRouter.of(context).go(navigateToRoute);
          }
        },
        child: Text(buttonText));

    AlertDialog dialog = AlertDialog(
      icon: (() {
        switch (type) {
          case OrderSubmissionStatusDialogType.pendingSubmit:
          case OrderSubmissionStatusDialogType.successfulSubmit:
            return const Center(
                child: SizedBox(width: 20, height: 20, child: CircularProgressIndicator()));
          case OrderSubmissionStatusDialogType.failedFill:
          case OrderSubmissionStatusDialogType.failedSubmit:
            return const Icon(
              Icons.cancel,
              color: Colors.red,
            );
          case OrderSubmissionStatusDialogType.filled:
            return const Icon(
              Icons.check_circle,
              color: Colors.green,
            );
        }
      })(),
      title: Text("$title ${(() {
        switch (type) {
          case OrderSubmissionStatusDialogType.pendingSubmit:
          case OrderSubmissionStatusDialogType.successfulSubmit:
            return "Pending";
          case OrderSubmissionStatusDialogType.filled:
            return "Success";
          case OrderSubmissionStatusDialogType.failedSubmit:
          case OrderSubmissionStatusDialogType.failedFill:
            return "Failure";
        }
      })()}"),
      content: content,
      actions: isPending ? null : [closeButton],
      insetPadding: insetPadding,
    );

    // If pending, prevent use of back button
    if (isPending) {
      return WillPopScope(child: dialog, onWillPop: () async => false);
    } else {
      return dialog;
    }
  }
}
