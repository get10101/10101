import 'dart:async';

import 'package:confetti/confetti.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/common/domain/background_task.dart';
import 'package:go_router/go_router.dart';

class TaskStatusDialog extends StatefulWidget {
  final String title;
  final TaskStatus status;
  final Widget content;
  final String buttonText;
  final EdgeInsets insetPadding;
  final String navigateToRoute;

  const TaskStatusDialog(
      {super.key,
      required this.title,
      required this.status,
      required this.content,
      this.buttonText = "Close",
      this.insetPadding = const EdgeInsets.all(50),
      this.navigateToRoute = ""});

  @override
  State<TaskStatusDialog> createState() => _TaskStatusDialog();
}

class _TaskStatusDialog extends State<TaskStatusDialog> {
  late final ConfettiController _confettiController;
  Timer? _timeout;

  bool timeout = false;

  @override
  void initState() {
    super.initState();
    _confettiController = ConfettiController(duration: const Duration(seconds: 3));
  }

  @override
  void dispose() {
    _confettiController.dispose();
    _timeout?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    bool isPending = widget.status == TaskStatus.pending;

    if (_timeout != null) {
      // cancel already running timeout timer if we receive a new update.
      _timeout!.cancel();
    }

    if (isPending) {
      // Start timeout showing the close button after 30 seconds.
      _timeout = Timer(const Duration(seconds: 30), () {
        setState(() {
          timeout = true;
        });
      });
    }

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
        switch (widget.status) {
          case TaskStatus.pending:
            return const Center(
                child: SizedBox(width: 20, height: 20, child: CircularProgressIndicator()));
          case TaskStatus.failed:
            return const Icon(
              Icons.cancel,
              color: Colors.red,
            );
          case TaskStatus.success:
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
                    maxBlastForce: 10,
                    // set a lower max blast force
                    minBlastForce: 9,
                    // set a lower min blast force
                    emissionFrequency: 0.00001,
                    numberOfParticles: 20,
                    // a lot of particles at once
                    gravity: 0.2,
                    shouldLoop: false,
                  ),
                ]);
        }
      })(),
      title: Text("${widget.title} ${(() {
        switch (widget.status) {
          case TaskStatus.pending:
            return "Pending";
          case TaskStatus.success:
            return "Success";
          case TaskStatus.failed:
            return "Failure";
        }
      })()}"),
      content: widget.content,
      actions: isPending && !timeout ? null : [closeButton],
      insetPadding: widget.insetPadding,
    );

    // If pending, prevent use of back button
    if (isPending) {
      return PopScope(
        canPop: false,
        child: dialog,
      );
    } else {
      return dialog;
    }
  }
}
