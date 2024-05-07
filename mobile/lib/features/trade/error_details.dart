import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/application/clickable_help_text.dart';
import 'package:share_plus/share_plus.dart';

class ErrorDetails extends StatelessWidget {
  final String details;

  const ErrorDetails({super.key, required this.details});

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const Text(
          "Error details",
          style: TextStyle(fontSize: 15, fontWeight: FontWeight.bold),
        ),
        SizedBox(
          width: 300,
          height: 120,
          child: Container(
            padding: const EdgeInsets.all(5.0),
            color: Colors.grey.shade300,
            child: SingleChildScrollView(
              child: Column(
                children: [
                  Text(
                    getPrettyJSONString(details),
                    style: const TextStyle(fontSize: 15),
                  ),
                ],
              ),
            ),
          ),
        ),
        const SizedBox(height: 5),
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
        const SizedBox(height: 5),
        ClickableHelpText(
            text: "Please help us fix this issue and join our telegram group: ",
            style: DefaultTextStyle.of(context).style),
      ],
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
