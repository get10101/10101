import 'dart:async';

import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/domain/background_task.dart';
import 'package:get_10101/features/trade/error_details.dart';
import 'package:go_router/go_router.dart';

/// Define Animation Type
class AppAnim {
  /// Loading Animation
  static const loading = 'assets/loading.gif';

  /// Success Animation
  static const success = 'assets/success.gif';

  /// Error Animation
  static const error = 'assets/error.gif';

  /// Info Animation
  static const info = 'assets/info.gif';
}

class TaskStatusDialog extends StatefulWidget {
  final BackgroundTask task;
  final Widget content;
  final VoidCallback? onClose;
  final String? successAnim;
  final bool showSuccessTitle;

  const TaskStatusDialog({
    super.key,
    required this.task,
    required this.content,
    this.onClose,
    this.successAnim,
    this.showSuccessTitle = true,
  });

  @override
  State<TaskStatusDialog> createState() => _TaskStatusDialog();
}

class _TaskStatusDialog extends State<TaskStatusDialog> {
  Timer? _timeout;

  bool timeout = false;

  Image? coverImage;

  @override
  void dispose() {
    super.dispose();
    _timeout?.cancel();

    // we need to evict the image cache to ensure that the gif is re-run next time.
    coverImage?.image.evict();
  }

  @override
  Widget build(BuildContext context) {
    bool isPending = widget.task.status == TaskStatus.pending;

    // we need to evict the image cache to ensure that the gif is re-run next time.
    coverImage?.image.evict();

    coverImage = Image.asset(
      widget.successAnim != null && widget.task.status == TaskStatus.success
          ? widget.successAnim!
          : switch (widget.task.status) {
              TaskStatus.pending => AppAnim.loading,
              TaskStatus.failed => AppAnim.error,
              TaskStatus.success => AppAnim.success,
            },
      fit: BoxFit.cover,
    );

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

    Widget closeButton = SizedBox(
      width: MediaQuery.of(context).size.width * 0.65,
      child: ElevatedButton(
          onPressed: () {
            GoRouter.of(context).pop();

            if (widget.onClose != null) {
              widget.onClose!();
            }
          },
          style: ElevatedButton.styleFrom(
              shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
              padding: EdgeInsets.zero,
              backgroundColor: tenTenOnePurple),
          child: const Text("Close")),
    );

    AlertDialog dialog = AlertDialog(
      contentPadding: EdgeInsets.zero,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(18.0),
      ),
      content: Container(
        decoration: BoxDecoration(
          color: Colors.white,
          borderRadius: BorderRadius.circular(18.0),
        ),
        clipBehavior: Clip.antiAlias,
        width: MediaQuery.of(context).size.shortestSide,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Container(
              width: double.infinity,
              height: 110,
              clipBehavior: Clip.antiAlias,
              decoration: const BoxDecoration(
                color: Colors.white,
              ),
              child: coverImage,
            ),
            const SizedBox(height: 15),
            if (widget.showSuccessTitle) buildTitle(widget.task.status),
            Padding(
                padding: EdgeInsets.only(
                    top: 10.0, left: 15.0, right: 15.0, bottom: isPending ? 25.0 : 0.0),
                child: widget.content),
            if (widget.task.status == TaskStatus.failed && widget.task.error != null)
              Padding(
                padding: const EdgeInsets.only(top: 10.0, left: 15.0, right: 15.0),
                child: ErrorDetails(details: widget.task.error!),
              ),
            if (!isPending || timeout)
              Padding(
                padding: const EdgeInsets.only(top: 20.0, left: 15.0, right: 15.0, bottom: 15.0),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.end,
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    closeButton,
                  ],
                ),
              )
          ],
        ),
      ),
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

  Widget buildTitle(TaskStatus status) {
    return Text(
      '$status',
      textAlign: TextAlign.center,
      style: const TextStyle(
        color: Colors.black,
        fontSize: 20.0,
        fontWeight: FontWeight.bold,
      ),
    );
  }
}
