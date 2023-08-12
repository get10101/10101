import 'dart:io';
import 'dart:typed_data';

import 'package:device_info_plus/device_info_plus.dart';
import 'package:f_logs/model/flog/flog.dart';
import 'package:feedback/feedback.dart';
import 'package:flutter_email_sender/flutter_email_sender.dart';
import 'package:path_provider/path_provider.dart';
import 'package:url_launcher/url_launcher.dart';

Future<void> submitFeedback(UserFeedback feedback) async {
  final screenshotFilePath = await writeImageToStorage(feedback.screenshot);
  final logs = await FLog.exportLogs();

  final deviceInfoPlugin = DeviceInfoPlugin();
  String info = "";
  if (Platform.isAndroid) {
    final deviceInfo = await deviceInfoPlugin.androidInfo;
    info =
        "${deviceInfo.model}, Android ${deviceInfo.version.sdkInt}, Release: ${deviceInfo.version.release}";
  } else {
    final deviceInfo = await deviceInfoPlugin.iosInfo;
    info = "${deviceInfo.name}, ${deviceInfo.systemName} ${deviceInfo.systemVersion}";
  }

  const feedbackEmail = "contact@10101.finance";
  const subject = "10101 Feedback";
  final body = '${feedback.text}\n\n----------\n$info';

  final Email email = Email(
    body: body,
    subject: subject,
    recipients: [feedbackEmail],
    attachmentPaths: [screenshotFilePath, logs.path],
    isHTML: false,
  );
  try {
    await FlutterEmailSender.send(email);
  } on Exception catch (e) {
    // fallback to using mailto link
    // We cannot auto-attach images with this, but the user will still be able to provide text feedback
    // We add a message that adds the exception to the body text.
    final Uri emailLaunchUri = Uri(
      scheme: 'mailto',
      path: feedbackEmail,
      queryParameters: {
        'subject': subject,
        'body': "$body \n\n----------\n"
            "Could not auto-attach images. Had to fallback to mailto link because: $e"
      },
    );

    if (await canLaunchUrl(emailLaunchUri)) {
      await launchUrl(emailLaunchUri);
    } else {
      // If we are unable to use mailto we throw the original exception because it likely contains more useful information.
      // If canLaunchUrl returns false this means there is likely no mail application configured, or we are unable to read the intent
      rethrow;
    }
  }
}

Future<String> writeImageToStorage(Uint8List feedbackScreenshot) async {
  final Directory output = await getTemporaryDirectory();
  final String screenshotFilePath = '${output.path}/feedback.png';
  final File screenshotFile = File(screenshotFilePath);
  await screenshotFile.writeAsBytes(feedbackScreenshot);
  return screenshotFilePath;
}
