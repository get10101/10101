import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

class TenTenOneAppBar extends StatelessWidget {
  final String title;

  const TenTenOneAppBar({
    super.key,
    required this.title,
  });

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.center,
      children: [
        Expanded(
          child: Stack(
            children: [
              GestureDetector(
                child: Container(
                    alignment: AlignmentDirectional.topStart,
                    decoration: BoxDecoration(
                        color: Colors.transparent, borderRadius: BorderRadius.circular(10)),
                    width: 70,
                    child: const Icon(
                      Icons.arrow_back_ios_new_rounded,
                      size: 22,
                    )),
                onTap: () => GoRouter.of(context).pop(),
              ),
              Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Text(
                    title,
                    style: const TextStyle(fontWeight: FontWeight.w500, fontSize: 20),
                  ),
                ],
              ),
            ],
          ),
        ),
      ],
    );
  }
}
