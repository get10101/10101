import 'package:flutter/services.dart';
import 'package:get_10101/common/application/clickable_help_text.dart';
import 'package:get_10101/common/application/share_logs_button.dart';
import 'package:flutter/material.dart';

class ErrorScreen extends StatefulWidget {
  static const route = "/error";
  static const label = "Error";

  const ErrorScreen({Key? key}) : super(key: key);

  @override
  State<ErrorScreen> createState() => _ErrorScreenState();
}

class _ErrorScreenState extends State<ErrorScreen> {
  @override
  Widget build(BuildContext context) {
    return AnnotatedRegion<SystemUiOverlayStyle>(
        value: SystemUiOverlayStyle.dark,
        child: Scaffold(
          body: SafeArea(
            child: Container(
              padding: const EdgeInsets.only(top: 20, left: 20, right: 20),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Container(
                    margin: const EdgeInsets.all(10),
                    child: const Column(
                      children: [
                        SizedBox(height: 20),
                        Text(
                          "Failed to start 10101!",
                          style: TextStyle(fontSize: 22, fontWeight: FontWeight.w400),
                        ),
                        SizedBox(height: 40),
                        Icon(Icons.error_outline_rounded, color: Colors.red, size: 100),
                        SizedBox(height: 40),
                        ClickableHelpText(
                            text: "Please help us fix this issue and join our telegram group: ",
                            style: TextStyle(fontSize: 17, color: Colors.black87)),
                        SizedBox(height: 30),
                        ShareLogsButton(),
                      ],
                    ),
                  )
                ],
              ),
            ),
          ),
        ));
  }
}
