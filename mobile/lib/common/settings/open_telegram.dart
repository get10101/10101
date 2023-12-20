import 'package:flutter/material.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:url_launcher/url_launcher.dart';

openTelegram(BuildContext context) async {
  final telegramUri = Uri(scheme: "https", host: "t.me", path: "get10101");
  final messenger = ScaffoldMessenger.of(context);
  if (await canLaunchUrl(telegramUri)) {
    launchUrl(telegramUri, mode: LaunchMode.externalApplication);
  } else {
    showSnackBar(messenger, "Failed to open link");
  }
}
