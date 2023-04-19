import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

enum SubmissionStatusDialogType { pending, success, failure }

class SubmissionStatusDialog extends StatelessWidget {
  final String title;
  final SubmissionStatusDialogType type;
  final Widget content;
  final String buttonText;
  final EdgeInsets insetPadding;
  final String navigateToRoute;

  const SubmissionStatusDialog(
      {super.key,
      required this.title,
      required this.type,
      required this.content,
      this.buttonText = "Close",
      this.insetPadding = const EdgeInsets.all(50),
      this.navigateToRoute = ""});

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      icon: (() {
        switch (type) {
          case SubmissionStatusDialogType.pending:
            return const Center(
                child: SizedBox(width: 20, height: 20, child: CircularProgressIndicator()));
          case SubmissionStatusDialogType.success:
            return const Icon(
              Icons.check_circle,
              color: Colors.green,
            );
          case SubmissionStatusDialogType.failure:
            return const Icon(
              Icons.cancel,
              color: Colors.red,
            );
        }
      })(),
      title: Text("$title ${(() {
        switch (type) {
          case SubmissionStatusDialogType.pending:
            return "Pending";
          case SubmissionStatusDialogType.success:
            return "Success";
          case SubmissionStatusDialogType.failure:
            return "Failure";
        }
      })()}"),
      content: content,
      actions: [
        ElevatedButton(
            onPressed: () {
              GoRouter.of(context).pop();

              if (navigateToRoute.isNotEmpty) {
                GoRouter.of(context).go(navigateToRoute);
              }
            },
            child: Text(buttonText))
      ],
      insetPadding: insetPadding,
    );
  }
}
