import 'package:confetti/confetti.dart';
import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

enum StableSubmissionStatusDialogType {
  pendingSubmit,
  successfulSubmit,
  filled,
  failedFill,
  failedSubmit
}

class StableSubmissionStatusDialog extends StatefulWidget {
  final String title;
  final StableSubmissionStatusDialogType type;
  final Widget content;
  final String buttonText;
  final EdgeInsets insetPadding;
  final String navigateToRoute;

  const StableSubmissionStatusDialog(
      {super.key,
      required this.title,
      required this.type,
      required this.content,
      this.buttonText = "Close",
      this.insetPadding = const EdgeInsets.all(50),
      this.navigateToRoute = ""});

  @override
  State<StableSubmissionStatusDialog> createState() => _OrderSubmissionStatusDialog();
}

class _OrderSubmissionStatusDialog extends State<StableSubmissionStatusDialog> {
  late final ConfettiController _confettiController;

  @override
  void initState() {
    super.initState();
    _confettiController = ConfettiController(duration: const Duration(seconds: 3));
  }

  @override
  void dispose() {
    _confettiController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    bool isPending = widget.type == StableSubmissionStatusDialogType.successfulSubmit ||
        widget.type == StableSubmissionStatusDialogType.pendingSubmit;

    WidgetsBinding.instance.addPostFrameCallback((_) {
      _confettiController.play();
    });

    Widget closeButton = ElevatedButton(
        onPressed: () {
          GoRouter.of(context).pop();

          if (widget.navigateToRoute.isNotEmpty) {
            GoRouter.of(context).go(widget.navigateToRoute);
          }
        },
        child: Text(widget.buttonText));

    AlertDialog dialog = AlertDialog(
      icon: (() {
        switch (widget.type) {
          case StableSubmissionStatusDialogType.pendingSubmit:
          case StableSubmissionStatusDialogType.successfulSubmit:
            return const Center(
                child: SizedBox(width: 20, height: 20, child: CircularProgressIndicator()));
          case StableSubmissionStatusDialogType.failedFill:
          case StableSubmissionStatusDialogType.failedSubmit:
            return const Icon(
              Icons.cancel,
              color: Colors.red,
            );
          case StableSubmissionStatusDialogType.filled:
            return Row(
                crossAxisAlignment: CrossAxisAlignment.center,
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  const Icon(
                    Icons.check_circle,
                    color: Colors.green,
                  ),
                  ConfettiWidget(
                    confettiController: _confettiController,
                    blastDirectionality: BlastDirectionality.explosive,
                    maxBlastForce: 10, // set a lower max blast force
                    minBlastForce: 9, // set a lower min blast force
                    emissionFrequency: 0.00001,
                    numberOfParticles: 20, // a lot of particles at once
                    gravity: 0.2,
                    shouldLoop: false,
                  ),
                ]);
        }
      })(),
      title: Text("${widget.title} ${(() {
        switch (widget.type) {
          case StableSubmissionStatusDialogType.pendingSubmit:
          case StableSubmissionStatusDialogType.successfulSubmit:
            return "Pending";
          case StableSubmissionStatusDialogType.filled:
            return "Success";
          case StableSubmissionStatusDialogType.failedSubmit:
          case StableSubmissionStatusDialogType.failedFill:
            return "Failure";
        }
      })()}"),
      content: widget.content,
      actions: isPending ? null : [closeButton],
      insetPadding: widget.insetPadding,
    );

    // If pending, prevent use of back button
    if (isPending) {
      return WillPopScope(child: dialog, onWillPop: () async => false);
    } else {
      return dialog;
    }
  }
}
