import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/snack_bar.dart';

import 'package:go_router/go_router.dart';

class AppDetails extends StatefulWidget {
  const AppDetails(
      {super.key,
      required this.nodeId,
      required this.number,
      required this.version,
      required this.commitHash,
      required this.branch});

  final String nodeId, number, version, commitHash, branch;

  @override
  State<AppDetails> createState() => _AppDetailsState();
}

class _AppDetailsState extends State<AppDetails> {
  EdgeInsets margin = const EdgeInsets.all(10);

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Container(
          padding: const EdgeInsets.all(20),
          child: SafeArea(
            child: SingleChildScrollView(
              child: Column(
                children: [
                  Row(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      Row(
                        children: [
                          GestureDetector(
                            child: const Icon(
                              Icons.arrow_back_ios_new_rounded,
                              size: 22,
                            ),
                            onTap: () {
                              context.pop();
                            },
                          ),
                        ],
                      ),
                      const Expanded(
                        child: Row(
                          mainAxisAlignment: MainAxisAlignment.center,
                          children: [
                            Text(
                              "App Info",
                              style: TextStyle(fontWeight: FontWeight.w500, fontSize: 20),
                            ),
                            // shift the row the size of the icon into the middle so that it is properly centered.
                            SizedBox(width: 22)
                          ],
                        ),
                      )
                    ],
                  ),
                  const SizedBox(
                    height: 20,
                  ),
                  Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      const Text(
                        "NODE INFO",
                        style: TextStyle(color: Colors.grey, fontSize: 17),
                      ),
                      const SizedBox(
                        height: 10,
                      ),
                      Container(
                          decoration: BoxDecoration(
                              color: Colors.white, borderRadius: BorderRadius.circular(15)),
                          child: Column(
                            children: [moreInfo(context, title: "Node Id", info: widget.nodeId)],
                          ))
                    ],
                  ),
                  const SizedBox(height: 20),
                  Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      const Text(
                        "BUILD INFO",
                        style: TextStyle(color: Colors.grey, fontSize: 18),
                      ),
                      const SizedBox(
                        height: 10,
                      ),
                      Container(
                          decoration: BoxDecoration(
                              color: Colors.white, borderRadius: BorderRadius.circular(15)),
                          child: Column(
                            children: [
                              moreInfo(context,
                                  title: "Number", info: widget.number, showCopyButton: true),
                              moreInfo(context,
                                  title: "Version", info: widget.version, showCopyButton: true),
                              moreInfo(context, title: "Commit Hash", info: widget.commitHash),
                              moreInfo(context,
                                  title: "Branch", info: widget.branch, showCopyButton: true)
                            ],
                          ))
                    ],
                  ),
                ],
              ),
            ),
          )),
    );
  }
}

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
            !showCopyButton
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
        !showCopyButton
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
