import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/common/global_keys.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:url_launcher/url_launcher.dart';

class ClickableHelpText extends StatelessWidget {
  final String text;
  final TextStyle style;

  const ClickableHelpText({super.key, required this.text, required this.style});

  @override
  Widget build(BuildContext context) {
    return RichText(
      text: TextSpan(
        text: text,
        style: style,
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
