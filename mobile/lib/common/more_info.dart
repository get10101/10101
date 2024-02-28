import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/snack_bar.dart';

Widget moreInfo(BuildContext context,
    {required String title, required String info, bool showCopyButton = false}) {
  return Container(
    padding: const EdgeInsets.all(15),
    child: Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      children: [
        Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              title,
              style:
                  const TextStyle(fontSize: 17, fontWeight: FontWeight.w400, color: Colors.black),
            ),
            const SizedBox(
              height: 7,
            ),
            showCopyButton
                ? SizedBox(
                    width: MediaQuery.of(context).size.width - 100,
                    child: Text(
                      info,
                      style: TextStyle(
                          fontSize: 18, fontWeight: FontWeight.w300, color: Colors.grey.shade700),
                    ))
                : const SizedBox()
          ],
        ),
        showCopyButton
            ? GestureDetector(
                onTap: () async {
                  showSnackBar(ScaffoldMessenger.of(context), "Copied $info");
                  await Clipboard.setData(ClipboardData(text: info));
                },
                child: Icon(
                  Icons.copy,
                  size: 17,
                  color: tenTenOnePurple.shade800,
                ),
              )
            : Text(
                info,
                style: TextStyle(
                    fontSize: 18, fontWeight: FontWeight.w300, color: Colors.grey.shade700),
              )
      ],
    ),
  );
}
