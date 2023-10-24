import 'package:flutter/material.dart';
import 'package:get_10101/common/settings_screen.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:go_router/go_router.dart';
import 'package:get_10101/ffi.dart' as rust;

class ForceClose extends StatefulWidget {
  const ForceClose({
    super.key,
  });

  @override
  State<ForceClose> createState() => _ForceCloseState();
}

class _ForceCloseState extends State<ForceClose> {
  bool isCloseChannelButtonDisabled = false;

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
                              "Force-Close Channel ",
                              style: TextStyle(fontWeight: FontWeight.w500, fontSize: 20),
                            ),
                          ],
                        ),
                      )
                    ],
                  ),
                  const SizedBox(
                    height: 20,
                  ),
                  Container(
                    padding: const EdgeInsets.all(10),
                    decoration: BoxDecoration(borderRadius: BorderRadius.circular(15)),
                    child: RichText(
                      text: const TextSpan(
                        style: TextStyle(fontSize: 18, color: Colors.black, letterSpacing: 0.4),
                        children: [
                          TextSpan(
                            text: "Warning",
                            style: TextStyle(color: Colors.red, fontWeight: FontWeight.w600),
                          ),
                          TextSpan(
                            text:
                                ": Force-closing your channel should only be considered as a last resort if 10101 is not reachable.\n\n",
                          ),
                          TextSpan(
                              text:
                                  "It's always better to collaboratively close as it will also save transaction fees.\n\n"),
                          TextSpan(text: "If you "),
                          TextSpan(
                              text: "force-close",
                              style: TextStyle(color: Colors.red, fontWeight: FontWeight.w600)),
                          TextSpan(text: ", you will have to pay the fees for going on-chain.\n\n"),
                          TextSpan(
                              text:
                                  "Your funds can be claimed by your on-chain wallet after a while.\n\n"),
                        ],
                      ),
                    ),
                  ),
                  const SizedBox(
                    height: 20,
                  ),
                  GestureDetector(
                    onTap: isCloseChannelButtonDisabled
                        ? null
                        : () async {
                            setState(() {
                              isCloseChannelButtonDisabled = true;
                            });
                            final messenger = ScaffoldMessenger.of(context);
                            try {
                              ensureCanCloseChannel(context);
                              await rust.api.forceCloseChannel();
                            } catch (e) {
                              showSnackBar(
                                messenger,
                                e.toString(),
                              );
                            } finally {
                              setState(() {
                                isCloseChannelButtonDisabled = false;
                              });
                            }
                          },
                    child: Container(
                      padding: const EdgeInsets.all(10),
                      decoration: BoxDecoration(
                          borderRadius: BorderRadius.circular(15), color: Colors.white),
                      child: Row(
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          Icon(
                            Icons.close_rounded,
                            color: Colors.red.shade400,
                          ),
                          const SizedBox(
                            width: 20,
                          ),
                          Text(
                            "Force-Close Channel",
                            style: TextStyle(
                                color: Colors.red.shade400,
                                fontSize: 18,
                                fontWeight: FontWeight.w400),
                          )
                        ],
                      ),
                    ),
                  )
                ],
              ),
            ),
          )),
    );
  }
}
